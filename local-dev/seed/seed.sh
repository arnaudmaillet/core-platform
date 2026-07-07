#!/usr/bin/env bash
# Dev-data seed for the local fleet. Runs INSIDE the compose network so the
# Keycloak issuer (iss) it sees matches exactly what auth-server sees — the
# account's identity_id is `${iss}#${sub}`, so this must line up or login fails.
#
# Creates 3 users (Keycloak → Active account → profile), mutual follows, and
# ~5 published posts each. Idempotent: safe to re-run.
set -uo pipefail

KC="http://keycloak:8080"
REALM="core-platform"
CLIENT_ID="core-platform-auth"   # public client — no secret

ACCOUNT="account-server:50059"
PROFILE="profile-server:50052"
SGRAPH="social-graph-server:50053"
POST="post-server:50056"

# username -> handle:display metadata (password is "password" for all)
USERS=("alice" "bob" "carol")
declare -A HANDLE=( [alice]="alice" [bob]="bob" [carol]="carol" )
declare -A DISPLAY=( [alice]="Alice Anderson" [bob]="Bob Brown" [carol]="Carol Clark" )
declare -A EMAIL=( [alice]="alice@dev.local" [bob]="bob@dev.local" [carol]="carol@dev.local" )
declare -A PROFILE_ID   # username -> profile_id (filled below)

log() { printf '\033[36m[seed]\033[0m %s\n' "$*"; }
err() { printf '\033[31m[seed:ERR]\033[0m %s\n' "$*" >&2; }

b64url_decode() {
  local d="${1//-/+}"; d="${d//_//}"
  local pad=$(( ${#d} % 4 )); [ "$pad" -ne 0 ] && d="$d$(printf '=%.0s' $(seq $((4-pad))))"
  printf '%s' "$d" | base64 -d 2>/dev/null
}
jwt_claim() { # <token> <claim>
  b64url_decode "$(printf '%s' "$1" | cut -d. -f2)" | jq -r ".$2"
}

kc_token() { # <username> -> prints access_token
  curl -s --max-time 10 \
    -d "client_id=${CLIENT_ID}" \
    -d "grant_type=password" -d "scope=openid" \
    -d "username=$1" -d "password=password" \
    "${KC}/realms/${REALM}/protocol/openid-connect/token" | jq -r '.access_token // empty'
}

# grpcurl wrapper: <host:port> <method> <json> -> prints response (empty on error)
call() { grpcurl -plaintext -max-time 15 -d "$3" "$1" "$2" 2>/dev/null; }

wait_for() { # <host:port> <label>
  for _ in $(seq 1 60); do
    grpcurl -plaintext -max-time 5 "$1" list >/dev/null 2>&1 && { log "$2 reachable"; return 0; }
    sleep 2
  done
  err "$2 ($1) never became reachable"; return 1
}

log "waiting for Keycloak + services…"
for _ in $(seq 1 60); do
  curl -sf --max-time 5 "${KC}/realms/${REALM}/.well-known/openid-configuration" >/dev/null 2>&1 && break
  sleep 2
done
wait_for "$ACCOUNT" account || exit 1
wait_for "$PROFILE" profile || exit 1
wait_for "$SGRAPH"  social-graph || exit 1
wait_for "$POST"    post || exit 1

# ── 1. Accounts + profiles ────────────────────────────────────────────────────
for u in "${USERS[@]}"; do
  tok="$(kc_token "$u")"
  [ -z "$tok" ] && { err "no Keycloak token for $u — is the realm imported?"; exit 1; }
  iss="$(jwt_claim "$tok" iss)"; sub="$(jwt_claim "$tok" sub)"
  identity="${iss}#${sub}"
  log "$u: identity_id=${identity}"

  aid="$(call "$ACCOUNT" account.v1.AccountService/GetAccountByIdentityId \
        "$(jq -nc --arg i "$identity" '{identity_id:$i}')" | jq -r '.id // empty')"
  if [ -z "$aid" ]; then
    # CreateAccount's response echoes identity_id (not the real account id), so
    # re-fetch by identity to get the server-generated UUID.
    call "$ACCOUNT" account.v1.AccountService/CreateAccount \
      "$(jq -nc --arg i "$identity" --arg e "${EMAIL[$u]}" \
         '{identity_id:$i, email:$e, password_hash:"x", country_of_residence:"US"}')" >/dev/null
    aid="$(call "$ACCOUNT" account.v1.AccountService/GetAccountByIdentityId \
          "$(jq -nc --arg i "$identity" '{identity_id:$i}')" | jq -r '.id // empty')"
    log "$u: created account ${aid}"
  else
    log "$u: account exists ${aid}"
  fi
  [ -z "$aid" ] && { err "$u: could not create/resolve account"; exit 1; }

  # PendingVerification -> Active (ignore if already active)
  call "$ACCOUNT" account.v1.AccountService/VerifyEmail \
    "$(jq -nc --arg a "$aid" '{account_id:$a}')" >/dev/null

  # profile_id is generated server-side and CreateProfile's response echoes
  # account_id (not the real id), so resolve the real profile_id by listing the
  # account's profiles — works whether the profile pre-exists or was just created.
  pid="$(call "$PROFILE" profile.v1.ProfileService/ListProfilesByAccount \
        "$(jq -nc --arg a "$aid" '{account_id:$a, limit:1}')" | jq -r '.profiles[0].profileId // empty')"
  if [ -z "$pid" ]; then
    call "$PROFILE" profile.v1.ProfileService/CreateProfile \
      "$(jq -nc --arg a "$aid" --arg h "${HANDLE[$u]}" --arg d "${DISPLAY[$u]}" \
         '{account_id:$a, handle:$h, display_name:$d, bio:"seeded dev user", profile_kind:1, locale:"en"}')" >/dev/null
    pid="$(call "$PROFILE" profile.v1.ProfileService/ListProfilesByAccount \
          "$(jq -nc --arg a "$aid" '{account_id:$a, limit:1}')" | jq -r '.profiles[0].profileId // empty')"
    log "$u: created profile ${pid}"
  else
    log "$u: profile exists ${pid}"
  fi
  [ -z "$pid" ] && { err "$u: could not create/resolve profile"; exit 1; }
  PROFILE_ID[$u]="$pid"
done

# ── 2. Mutual follows (everyone follows everyone) ─────────────────────────────
for a in "${USERS[@]}"; do
  for b in "${USERS[@]}"; do
    [ "$a" = "$b" ] && continue
    call "$SGRAPH" social_graph.v1.SocialGraphService/Follow \
      "$(jq -nc --arg x "${PROFILE_ID[$a]}" --arg y "${PROFILE_ID[$b]}" '{actor_id:$x, target_id:$y}')" >/dev/null
  done
  log "$a follows the other users"
done

# ── 3. ~5 published posts each ────────────────────────────────────────────────
for u in "${USERS[@]}"; do
  pid="${PROFILE_ID[$u]}"
  have="$(call "$POST" post.v1.PostService/ListPostsByProfile \
         "$(jq -nc --arg p "$pid" '{profile_id:$p}')" | jq '[.posts[]?] | length' 2>/dev/null)"
  have="${have:-0}"
  for n in 1 2 3 4 5; do
    [ "$n" -le "$have" ] && continue
    postid="$(call "$POST" post.v1.PostService/CreatePost \
             "$(jq -nc --arg p "$pid" --arg c "Post #${n} from ${DISPLAY[$u]} — hello from the local fleet!" \
                '{profile_id:$p, kind:1, caption:$c}')" | jq -r '.postId // empty')"
    [ -z "$postid" ] && { err "$u: CreatePost #$n failed"; continue; }
    call "$POST" post.v1.PostService/PublishPost \
      "$(jq -nc --arg id "$postid" --arg p "$pid" '{post_id:$id, profile_id:$p}')" >/dev/null
  done
  log "$u: posts ensured (>=5)"
done

log "✅ seed complete — 3 users (alice/bob/carol, password: password), mutual follows, 5 posts each"
