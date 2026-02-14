// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { FsFile } from "ext:deno_fs/30_fs.js";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { getRid } from "ext:deno_node/internal/fs/fd_map.ts";

const {
  Error,
  PromisePrototypeThen,
  SymbolFor,
} = primordials;

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

  PromisePrototypeThen(
    new FsFile(getRid(fd), SymbolFor("Deno.internal.FsFile")).truncate(len),
    () => callback(null),
    callback,
  );
}

export function ftruncateSync(fd: number, len?: number) {
  new FsFile(getRid(fd), SymbolFor("Deno.internal.FsFile")).truncateSync(len);
}

export const ftruncatePromise = promisify(ftruncate) as (
  fd: number,
  len?: number,
) => Promise<void>;
