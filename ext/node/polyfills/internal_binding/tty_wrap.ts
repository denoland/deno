// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core } = __bootstrap;
const { TTY } = core.ops;

// Mark TTY as a StreamBase handle, matching Node's StreamBase::AddMethods.
TTY.prototype.isStreamBase = true;

return {
  TTY,
};
})();
