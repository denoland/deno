// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import type { CallbackWithError } from "./_fs_common.ts";
import { notImplemented } from "../_utils.ts";

/** Revist once https://github.com/denoland/deno/issues/4017 lands */

//TODO - 'path' can also be a Buffer.  Neither of these polyfills
//is available yet.  See https://github.com/denoland/deno/issues/3403
export function access(
  path: string | URL, // eslint-disable-line @typescript-eslint/no-unused-vars
  modeOrCallback: number | Function, // eslint-disable-line @typescript-eslint/no-unused-vars
  callback?: CallbackWithError, // eslint-disable-line @typescript-eslint/no-unused-vars
): void {
  notImplemented("Not yet available");
}

//TODO - 'path' can also be a Buffer.  Neither of these polyfills
//is available yet.  See https://github.com/denoland/deno/issues/3403
// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function accessSync(path: string | URL, mode?: number): void {
  notImplemented("Not yet available");
}
