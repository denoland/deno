// Copyright 2018-2025 the Deno authors. MIT license.

import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { validateInt32 } from "ext:deno_node/internal/validators.mjs";
import { primordials } from "ext:core/mod.js";

const { PromisePrototypeThen, SymbolFor } = primordials;

export function fsync(
  fd: number,
  callback: CallbackWithError,
) {
  validateInt32(fd, "fd", 0);
  PromisePrototypeThen(
    new FsFile(fd, SymbolFor("Deno.internal.FsFile")).sync(),
    () => callback(null),
    callback,
  );
}

export function fsyncSync(fd: number) {
  validateInt32(fd, "fd", 0);
  new FsFile(fd, SymbolFor("Deno.internal.FsFile")).syncSync();
}

export const fsyncPromise = promisify(fsync) as (fd: number) => Promise<void>;
