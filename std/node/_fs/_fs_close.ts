// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { CallbackWithError } from "./_fs_common.ts";

export function close(fd: number, callback: CallbackWithError): void {
  new Promise((resolve, reject) => {
    try {
      Deno.close(fd);
      resolve();
    } catch (err) {
      reject(err);
    }
  })
    .then(() => {
      callback();
    })
    .catch((err) => {
      callback(err);
    });
}

export function closeSync(fd: number): void {
  Deno.close(fd);
}
