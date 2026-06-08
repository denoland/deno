// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = __bootstrap;
// The cppgc `Stats` object + throwIfNoEntry suppression are produced in Rust
// (ext/node/ops/fs.rs); the op returns the cppgc `Stats` directly. Errors carry
// `os_errno`; denoErrorToNodeError derives the node `code`/message.
const { op_node_fs_lstat, op_node_fs_lstat_sync } = core.ops;
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");
const { callbackifyOpt } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_common.ts",
);

// The op extracts bigint/throwIfNoEntry from options, validates the path
// (async(eager_throw)), and resolves the Stats (or undefined when
// throwIfNoEntry is false and the path is missing).
const lstat = callbackifyOpt(op_node_fs_lstat);

const lstatPromise = promisify(lstat);

// Direct op binding: the op extracts bigint/throwIfNoEntry from options,
// validates the path, and returns the cppgc Stats (or undefined when
// throwIfNoEntry is false and the path is missing).
const lstatSync = op_node_fs_lstat_sync;

return { lstat, lstatPromise, lstatSync };
})();
