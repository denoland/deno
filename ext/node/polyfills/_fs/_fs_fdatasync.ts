// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";

export function fdatasync(
  fd: number,
  callback: CallbackWithError,
) {
  Deno.fdatasync(fd).then(() => callback(null), callback);
}

export function fdatasyncSync(fd: number) {
  Deno.fdatasyncSync(fd);
}
