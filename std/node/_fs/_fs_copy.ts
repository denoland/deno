// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { CallbackWithError } from "./_fs_common.ts";

export function copyFile(
  source: string,
  destination: string,
  callback: CallbackWithError
): void {
  new Promise(async (resolve, reject) => {
    try {
      await Deno.copyFile(source, destination);
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

export function copyFileSync(source: string, destination: string): void {
  try {
    Deno.copyFileSync(source, destination);
  } catch (err) {
    throw err;
  }
}
