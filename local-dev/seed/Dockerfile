# Tiny seed runner: grpcurl + curl + jq + bash. Runs in the compose network so
# the Keycloak issuer it observes matches auth-server's view (see seed.sh).
FROM alpine:3.20
RUN apk add --no-cache bash curl jq
# grpcurl static binary from the official image.
COPY --from=fullstorydev/grpcurl:v1.9.1 /bin/grpcurl /usr/local/bin/grpcurl
COPY seed.sh /usr/local/bin/seed.sh
RUN chmod +x /usr/local/bin/seed.sh
ENTRYPOINT ["/usr/local/bin/seed.sh"]
