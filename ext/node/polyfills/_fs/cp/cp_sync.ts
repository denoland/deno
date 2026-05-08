// Copyright 2018-2026 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

(function () {
const { core } = globalThis.__bootstrap;
const {
  denoErrorToNodeError,
  ERR_INVALID_RETURN_VALUE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { op_node_cp_sync } = core.ops;
const { throwCpError } = core.loadExtScript("ext:deno_node/_fs/cp/cp.ts");

const {
  isPromise,
} = core;

function cpSyncFn(
  src,
  dest,
  opts,
) {
  try {
    if (opts.filter) {
      // deno-lint-ignore prefer-primordials
      const shouldCopy = opts.filter(src, dest);
      if (isPromise(shouldCopy)) {
        throw new ERR_INVALID_RETURN_VALUE("boolean", "filter", shouldCopy);
      }
      if (!shouldCopy) return;
    }

    op_node_cp_sync(
      src,
      dest,
      opts.dereference,
      opts.recursive,
      opts.force,
      opts.errorOnExist,
      opts.preserveTimestamps,
      opts.verbatimSymlinks,
      opts.mode ?? 0,
      opts.filter,
    );
  } catch (err) {
    if (typeof err?.os_errno === "number") {
      throw denoErrorToNodeError(err, {
        path: err.path,
        dest: err.dest,
        syscall: err.syscall,
      });
    }

    throwCpError(err);
  }
}

return { cpSyncFn };
})();
