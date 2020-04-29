// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { CallbackWithError } from "./_fs_common.ts";

export function copyFile(
  source: string,
  destination: string,
  callback: CallbackWithError
): void {
  Deno.copyFile(source, destination)
    .then(() => callback())
    .catch(callback);
}

export function copyFileSync(source: string, destination: string): void {
  Deno.copyFileSync(source, destination);
}
