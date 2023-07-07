// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { getValidatedFd } from "ext:deno_node/internal/fs/utils.mjs";

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
