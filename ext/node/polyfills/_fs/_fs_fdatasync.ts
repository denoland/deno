// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import * as denoFs from "ext:deno_fs/30_fs.js";

export function fdatasync(
  fd: number,
  callback: CallbackWithError,
) {
  denoFs.fdatasync(fd).then(() => callback(null), callback);
}

export function fdatasyncSync(fd: number) {
  denoFs.fdatasyncSync(fd);
}
