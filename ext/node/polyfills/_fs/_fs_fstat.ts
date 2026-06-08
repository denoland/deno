// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
// fstat returns the cppgc `Stats` directly with the bigint variant handled in
// Rust (ext/node/ops/fs.rs); errors carry `os_errno` and denoErrorToNodeError
// derives the node `code`/message.
const { op_node_fs_fstat_stats, op_node_fs_fstat_stats_sync } = core.ops;
const { callbackifyOpt } = core.loadExtScript(
  "ext:deno_node/_fs/_fs_common.ts",
);

const { Promise } = primordials;

// The op validates the fd (getValidatedFd) + extracts bigint from options and
// returns the cppgc Stats (errors node-formatted with syscall "fstat").
const fstat = callbackifyOpt(op_node_fs_fstat_stats);

// Direct op binding: the op validates the fd, extracts bigint from options,
// reads the stats, and node-formats errors (syscall "fstat").
const fstatSync = op_node_fs_fstat_stats_sync;

function fstatPromise(
  fd,
  options,
) {
  return new Promise((resolve, reject) => {
    fstat(fd, options, (err, stats) => {
      if (err) reject(err);
      else resolve(stats);
    });
  });
}

return { fstat, fstatSync, fstatPromise };
})();
