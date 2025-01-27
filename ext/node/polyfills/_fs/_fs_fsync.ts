// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { promisify } from "ext:deno_node/internal/util.mjs";

export function fsync(
  fd: number,
  callback: CallbackWithError,
) {
  new FsFile(fd, Symbol.for("Deno.internal.FsFile")).sync().then(
    () => callback(null),
    callback,
  );
}

export function fsyncSync(fd: number) {
  new FsFile(fd, Symbol.for("Deno.internal.FsFile")).syncSync();
}

export const fsyncPromise = promisify(fsync) as (fd: number) => Promise<void>;
