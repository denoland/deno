// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";

export function fsync(
  fd: number,
  callback: CallbackWithError,
) {
  Deno.fsync(fd).then(() => callback(null), callback);
}

export function fsyncSync(fd: number) {
  Deno.fsyncSync(fd);
}
