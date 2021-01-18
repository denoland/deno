// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "./_fs_common.ts";

export function close(fd: number, callback: CallbackWithError): void {
  queueMicrotask(() => {
    try {
      Deno.close(fd);
      callback(null);
    } catch (err) {
      callback(err);
    }
  });
}

export function closeSync(fd: number): void {
  Deno.close(fd);
}
