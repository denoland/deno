// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "./_fs_common.ts";

type Path = string; // TODO path can also be a Buffer or URL.
type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

export function mkdir(
  path: Path,
  options?: MkdirOptions | CallbackWithError,
  callback?: CallbackWithError
): void {
  let mode = 0o777;
  let recursive = false;

  if (typeof options == "function") {
    callback == options;
  } else if (typeof options === "number") {
    mode = options;
  } else if (typeof options === "boolean") {
    recursive = options;
  } else if (options) {
    if (options.recursive !== undefined) recursive = options.recursive;
    if (options.mode !== undefined) mode = options.mode;
  }
  if (typeof recursive !== "boolean")
    throw new Deno.errors.InvalidData(
      "invalid recursive option , must be a boolean"
    );
  new Promise(async (resolve, reject) => {
    try {
      await Deno.mkdir(path, { recursive, mode });
      resolve();
    } catch (err) {
      reject(err);
    }
  })
    .then(() => {
      if (callback && typeof callback == "function") {
        callback();
      }
    })
    .catch((err) => {
      if (callback && typeof callback == "function") {
        callback(err);
      }
    });
}

export function mkdirSync(path: Path, options?: MkdirOptions): void {
  let mode = 0o777;
  let recursive = false;

  if (typeof options === "number") {
    mode = options;
  } else if (typeof options === "boolean") {
    recursive = options;
  } else if (options) {
    if (options.recursive !== undefined) recursive = options.recursive;
    if (options.mode !== undefined) mode = options.mode;
  }
  if (typeof recursive !== "boolean")
    throw new Deno.errors.InvalidData(
      "invalid recursive option , must be a boolean"
    );
  try {
    Deno.mkdirSync(path, { recursive, mode });
  } catch (err) {
    throw err;
  }
}
