// Copyright 2018-2026 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

import { type CopySyncOptions } from "node:fs";
const {
  denoErrorToNodeError,
  ERR_INVALID_RETURN_VALUE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
import { core } from "ext:core/mod.js";
import { op_node_cp_sync } from "ext:core/ops";
import { throwCpError } from "ext:deno_node/_fs/cp/cp.ts";

const {
  isPromise,
} = core;

export function cpSyncFn(
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
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
      throw denoErrorToNodeError(err as Error, {
        path: err.path,
        dest: err.dest,
        syscall: err.syscall,
      });
    }

    throwCpError(err);
  }
}
