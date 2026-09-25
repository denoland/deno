// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;

// Public entry point for the iterable streams API.
// Usage: require('stream/iter') or require('node:stream/iter')
// Requires: --experimental-stream-iter

const {
  ObjectFreeze,
} = primordials;

const { emitExperimentalWarning } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
emitExperimentalWarning("stream/iter");

// Protocol symbols
const {
  toStreamable,
  toAsyncStreamable,
  broadcastProtocol,
  shareProtocol,
  shareSyncProtocol,
  drainableProtocol,
} = core.loadExtScript("ext:deno_node/internal/streams/iter/types.js");

// Factories
const { push } = core.loadExtScript(
  "ext:deno_node/internal/streams/iter/push.js",
);
const { duplex } = core.loadExtScript(
  "ext:deno_node/internal/streams/iter/duplex.js",
);
const { from, fromSync } = core.loadExtScript(
  "ext:deno_node/internal/streams/iter/from.js",
);

// Pipelines
const {
  pull,
  pullSync,
  pipeTo,
  pipeToSync,
} = core.loadExtScript("ext:deno_node/internal/streams/iter/pull.js");

// Consumers
const {
  bytes,
  bytesSync,
  text,
  textSync,
  arrayBuffer,
  arrayBufferSync,
  array,
  arraySync,
  tap,
  tapSync,
  merge,
  ondrain,
} = core.loadExtScript("ext:deno_node/internal/streams/iter/consumers.js");

// Classic stream interop (Node.js-specific, not part of the spec)
const {
  fromReadable,
  fromWritable,
  toReadable,
  toReadableSync,
  toWritable,
} = core.loadExtScript("ext:deno_node/internal/streams/iter/classic.js");

// Multi-consumer
const { broadcast, Broadcast } = core.loadExtScript(
  "ext:deno_node/internal/streams/iter/broadcast.js",
);
const {
  share,
  shareSync,
  Share,
  SyncShare,
} = core.loadExtScript("ext:deno_node/internal/streams/iter/share.js");

/**
 * Stream namespace - unified access to all stream functions.
 * @example
 * const { Stream } = require('stream/iter');
 *
 * const { writer, readable } = Stream.push();
 * await writer.write("hello");
 * await writer.end();
 *
 * const output = Stream.pull(readable, transform1, transform2);
 * const data = await Stream.bytes(output);
 */
const Stream = ObjectFreeze({
  // Factories
  push,
  duplex,
  from,
  fromSync,

  // Pipelines
  pull,
  pullSync,

  // Pipe to destination
  pipeTo,
  pipeToSync,

  // Consumers (async)
  bytes,
  text,
  arrayBuffer,
  array,

  // Consumers (sync)
  bytesSync,
  textSync,
  arrayBufferSync,
  arraySync,

  // Combining
  merge,

  // Multi-consumer (push model)
  broadcast,

  // Multi-consumer (pull model)
  share,
  shareSync,

  // Utilities
  tap,
  tapSync,

  // Drain utility for event source integration
  ondrain,

  // Protocol symbols
  toStreamable,
  toAsyncStreamable,
  broadcastProtocol,
  shareProtocol,
  shareSyncProtocol,
  drainableProtocol,
});

return {
  // The Stream namespace
  Stream,

  // Also export everything individually for destructured imports

  // Protocol symbols
  toStreamable,
  toAsyncStreamable,
  broadcastProtocol,
  shareProtocol,
  shareSyncProtocol,
  drainableProtocol,

  // Factories
  push,
  duplex,
  from,
  fromSync,

  // Pipelines
  pull,
  pullSync,
  pipeTo,
  pipeToSync,

  // Consumers (async)
  bytes,
  text,
  arrayBuffer,
  array,

  // Consumers (sync)
  bytesSync,
  textSync,
  arrayBufferSync,
  arraySync,

  // Combining
  merge,

  // Multi-consumer
  broadcast,
  Broadcast,
  share,
  shareSync,
  Share,
  SyncShare,

  // Utilities
  tap,
  tapSync,
  ondrain,

  // Classic stream interop
  fromReadable,
  fromWritable,
  toReadable,
  toReadableSync,
  toWritable,
};
})();
