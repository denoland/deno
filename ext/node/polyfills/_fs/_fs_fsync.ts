// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  type CallbackWithError,
} from "ext:deno_node/_fs/_fs_common.ts";
import { validateInt32 } from "ext:deno_node/internal/validators.mjs";
import { FsFile } from "ext:deno_fs/30_fs.js";

const {
  PromisePrototypeThen,
  SymbolFor,
} = primordials;

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
