// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

type ExitsCallback = (exists: boolean) => void;

/* Deprecated in node api */

export function exists(path: string, callback: ExitsCallback): void {
  new Promise(async (resolve, reject) => {
    try {
      await Deno.lstat(path);
      resolve();
    } catch (err) {
      reject(err);
    }
  })
    .then(() => {
      callback(true);
    })
    .catch(() => {
      callback(false);
    });
}

export function existsSync(path: string): boolean {
  try {
    Deno.lstatSync(path);
    return true;
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return false;
    }
    throw err;
  }
}
