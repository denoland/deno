// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import * as denoFs from "ext:deno_fs/30_fs.js";

export function ftruncate(
  fd: number,
  lenOrCallback: number | CallbackWithError,
  maybeCallback?: CallbackWithError,
) {
  const len: number | undefined = typeof lenOrCallback === "number"
    ? lenOrCallback
    : undefined;
  const callback: CallbackWithError = typeof lenOrCallback === "function"
    ? lenOrCallback
    : maybeCallback as CallbackWithError;

  if (!callback) throw new Error("No callback function supplied");

  denoFs.ftruncate(fd, len).then(() => callback(null), callback);
}

export function ftruncateSync(fd: number, len?: number) {
  denoFs.ftruncateSync(fd, len);
}
