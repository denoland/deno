// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { promisify } from "ext:deno_node/internal/util.mjs";

export function fdatasync(
  fd: number,
  callback: CallbackWithError,
) {
  new FsFile(fd, Symbol.for("Deno.internal.FsFile")).syncData().then(
    () => callback(null),
    callback,
  );
}

export function fdatasyncSync(fd: number) {
  new FsFile(fd, Symbol.for("Deno.internal.FsFile")).syncDataSync();
}

export const fdatasyncPromise = promisify(fdatasync) as (
  fd: number,
) => Promise<void>;
