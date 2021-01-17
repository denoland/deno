// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "./_fs_common.ts";
import { notImplemented } from "../_utils.ts";

/** Revist once https://github.com/denoland/deno/issues/4017 lands */

// TODO(bartlomieju) 'path' can also be a Buffer.  Neither of these polyfills
//is available yet.  See https://github.com/denoland/deno/issues/3403
export function access(
  _path: string | URL,
  _modeOrCallback: number | ((...args: unknown[]) => void),
  _callback?: CallbackWithError,
): void {
  notImplemented("Not yet available");
}

// TODO(bartlomieju) 'path' can also be a Buffer.  Neither of these polyfills
// is available yet.  See https://github.com/denoland/deno/issues/3403
// eslint-disable-next-line @typescript-eslint/no-unused-vars
export function accessSync(path: string | URL, mode?: number): void {
  notImplemented("Not yet available");
}
