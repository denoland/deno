// Copyright 2018-2026 the Deno authors. MIT license.
// deno-fmt-ignore-file
(function () {
  const { core } = globalThis.__bootstrap;
  const { TTY } = core.ops;

  // Mark TTY as a StreamBase handle, matching Node's StreamBase::AddMethods.
  TTY.prototype.isStreamBase = true;

  return {
    TTY,
  };
})()
