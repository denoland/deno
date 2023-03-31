// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import * as denoFs from "ext:deno_fs/30_fs.js";

export function fsync(
  fd: number,
  callback: CallbackWithError,
) {
  denoFs.fsync(fd).then(() => callback(null), callback);
}

export function fsyncSync(fd: number) {
  denoFs.fsyncSync(fd);
}
