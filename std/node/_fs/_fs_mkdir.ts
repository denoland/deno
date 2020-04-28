// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { CallbackWithError } from "./_fs_common.ts";

/**
 * TODO: Also accept 'path' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
type Path = string;
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
    callback = options;
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
  Deno.mkdir(path, { recursive, mode })
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
