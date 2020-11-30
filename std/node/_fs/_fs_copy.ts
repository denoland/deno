// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { fromFileUrl } from "../path.ts";
import type { CallbackWithError } from "./_fs_common.ts";

export function copyFile(
  source: string | URL,
  destination: string,
  callback: CallbackWithError,
): void {
  source = source instanceof URL ? fromFileUrl(source) : source;

  Deno.copyFile(source, destination)
    .then(() => callback())
    .catch(callback);
}

export function copyFileSync(source: string | URL, destination: string): void {
  source = source instanceof URL ? fromFileUrl(source) : source;
  Deno.copyFileSync(source, destination);
}
