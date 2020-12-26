// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import type { CallbackWithError } from "./_fs_common.ts";

export function close(fd: number, callback: CallbackWithError): void {
  queueMicrotask(() => {
    let error = null;
    try {
      Deno.close(fd);
    } catch (err) {
      error = err;
    }
    callback(error);
  });
}

export function closeSync(fd: number): void {
  Deno.close(fd);
}
