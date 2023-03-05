// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  CallbackWithError,
  isFd,
  maybeCallback,
  WriteFileOptions,
} from "internal:deno_node/_fs/_fs_common.ts";
import { Encodings } from "internal:deno_node/_utils.ts";
import {
  copyObject,
  getOptions,
} from "internal:deno_node/internal/fs/utils.mjs";
import {
  writeFile,
  writeFileSync,
} from "internal:deno_node/_fs/_fs_writeFile.ts";
import { promisify } from "internal:deno_node/internal/util.mjs";

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function appendFile(
  path: string | number | URL,
  data: string | Uint8Array,
  options: Encodings | WriteFileOptions | CallbackWithError,
  callback?: CallbackWithError,
) {
  callback = maybeCallback(callback || options);
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  // Don't make changes directly on options object
  options = copyObject(options);

  // Force append behavior when using a supplied file descriptor
  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFile(path, data, options, callback);
}

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export const appendFilePromise = promisify(appendFile) as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function appendFileSync(
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) {
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  // Don't make changes directly on options object
  options = copyObject(options);

  // Force append behavior when using a supplied file descriptor
  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFileSync(path, data, options);
}
