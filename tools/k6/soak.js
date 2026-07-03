// tools/k6/soak.js — staging fleet soak: one synthetic user journey per
// iteration, gRPC via server reflection (every fleet server registers it), so
// no proto files ship with the script.
//
//   CreatePost → PublishPost → UpsertReaction → CreateComment → GetFollowingFeed
//
// What it exercises beyond boot health: Scylla writes/reads (post, comment,
// timeline), Redis (engagement Lua, timeline cache), the Kafka fan-out
// (post.v1.events / engagement.reactions / comment.created → timeline, search,
// realtime, counter, notification consumers), KEDA lag scaling on the workers,
// and the gRPC connection-recycling behavior under sustained load.
//
// NETWORK: post/profile/etc. are tightened mesh callees — the runner pods must
// carry the `app: k6-loadtest` label (see testrun-soak.yaml) to pass the
// staging `allow-loadtest-to-mesh` NetworkPolicy.
//
// IDs are UUIDv7 (the fleet's id convention) generated inline — the script is
// dependency-free on purpose so `k6 archive` needs no network.
//
// Run: see tools/k6/testrun-soak.yaml.

import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';

const POST = 'staging-post-server:50056';
const ENGAGEMENT = 'staging-engagement-server:50058';
const COMMENT = 'staging-comment-server:50057';
const TIMELINE = 'staging-timeline-server:50070';

export const options = {
  scenarios: {
    journeys: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '2m', target: 10 },
        { duration: '10m', target: 10 },
        { duration: '1m', target: 0 },
      ],
      gracefulRampDown: '30s',
    },
  },
  thresholds: {
    // Observational soak: record, don't abort. The check rate still fails the
    // run's exit code if the fleet seriously misbehaves.
    checks: ['rate>0.90'],
    grpc_req_duration: ['p(95)<1500'],
  },
};

function uuidv7() {
  const ts = Date.now();
  const bytes = new Uint8Array(16);
  // 48-bit big-endian unix-ms timestamp.
  bytes[0] = (ts / 2 ** 40) & 0xff;
  bytes[1] = (ts / 2 ** 32) & 0xff;
  bytes[2] = (ts / 2 ** 24) & 0xff;
  bytes[3] = (ts / 2 ** 16) & 0xff;
  bytes[4] = (ts / 2 ** 8) & 0xff;
  bytes[5] = ts & 0xff;
  for (let i = 6; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
  bytes[6] = (bytes[6] & 0x0f) | 0x70; // version 7
  bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC 4122 variant
  const h = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

const post = new grpc.Client();
const engagement = new grpc.Client();
const comment = new grpc.Client();
const timeline = new grpc.Client();
let connected = false;

export default function () {
  if (!connected) {
    // Reflection: schemas come from the servers themselves.
    post.connect(POST, { plaintext: true, reflect: true });
    engagement.connect(ENGAGEMENT, { plaintext: true, reflect: true });
    comment.connect(COMMENT, { plaintext: true, reflect: true });
    timeline.connect(TIMELINE, { plaintext: true, reflect: true });
    connected = true;
  }

  const profileId = uuidv7();

  const created = post.invoke('post.v1.PostService/CreatePost', {
    profile_id: profileId,
    kind: 'POST_KIND_TEXT_ONLY',
    caption: `soak ${new Date().toISOString()}`,
  });
  const createdOk = check(created, {
    'CreatePost OK': (r) => r && r.status === grpc.StatusOK,
  });
  if (!createdOk) {
    sleep(1);
    return;
  }
  const postId = created.message.postId;

  const published = post.invoke('post.v1.PostService/PublishPost', {
    post_id: postId,
    profile_id: profileId,
  });
  check(published, { 'PublishPost OK': (r) => r && r.status === grpc.StatusOK });

  const reacted = engagement.invoke('engagement.v1.EngagementService/UpsertReaction', {
    post_id: postId,
    profile_id: uuidv7(), // a different synthetic user reacts
    kind: 'REACTION_KIND_HEART',
  });
  check(reacted, { 'UpsertReaction OK': (r) => r && r.status === grpc.StatusOK });

  const commented = comment.invoke('comment.v1.CommentService/CreateComment', {
    comment_id: uuidv7(),
    post_id: postId,
    author_id: uuidv7(),
    body: 'soak comment',
  });
  check(commented, { 'CreateComment OK': (r) => r && r.status === grpc.StatusOK });

  const feed = timeline.invoke('timeline.v1.TimelineService/GetFollowingFeed', {
    profile_id: profileId,
    limit: 20,
  });
  check(feed, { 'GetFollowingFeed OK': (r) => r && r.status === grpc.StatusOK });

  sleep(0.5);
}
