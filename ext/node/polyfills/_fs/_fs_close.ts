// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "internal:deno_node/polyfills/_fs/_fs_common.ts";
import { getValidatedFd } from "internal:deno_node/polyfills/internal/fs/utils.mjs";

export function close(fd: number, callback: CallbackWithError) {
  fd = getValidatedFd(fd);
  setTimeout(() => {
    let error = null;
    try {
      Deno.close(fd);
    } catch (err) {
      error = err instanceof Error ? err : new Error("[non-error thrown]");
    }
    callback(error);
  }, 0);
}

export function closeSync(fd: number) {
  fd = getValidatedFd(fd);
  Deno.close(fd);
}
