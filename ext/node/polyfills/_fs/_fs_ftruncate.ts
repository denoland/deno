// Copyright 2018-2025 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";

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
    : (maybeCallback as CallbackWithError);

  if (!callback) throw new Error("No callback function supplied");

  new FsFile(fd, Symbol.for("Deno.internal.FsFile"))
    .truncate(len)
    .then(() => callback(null), callback);
}

export function ftruncateSync(fd: number, len?: number) {
  new FsFile(fd, Symbol.for("Deno.internal.FsFile")).truncateSync(len);
}

export function ftruncatePromise(fd: number, len?: number): Promise<void> {
  return new Promise((resolve, reject) => {
    ftruncate(fd, len, (err) => {
      if (err) reject(err);
      else resolve();
    });
  });
}
