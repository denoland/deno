// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "./_fs_common.ts";
import { fromFileUrl } from "../path.ts";

export function copyFile(
  source: string | URL,
  destination: string,
  callback: CallbackWithError,
): void {
  source = source instanceof URL ? fromFileUrl(source) : source;

  Deno.copyFile(source, destination).then(() => callback(null), callback);
}

export function copyFileSync(source: string | URL, destination: string): void {
  source = source instanceof URL ? fromFileUrl(source) : source;
  Deno.copyFileSync(source, destination);
}
