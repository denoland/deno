// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

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
