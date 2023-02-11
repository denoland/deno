// Copyright the Browserify authors. MIT License.
// Ported mostly from https://github.com/browserify/path-browserify/
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { isWindows } from "internal:deno_node/polyfills/_util/os.ts";
import _win32 from "internal:deno_node/polyfills/path/win32.ts";
import _posix from "internal:deno_node/polyfills/path/posix.ts";

const path = isWindows ? _win32 : _posix;

export const win32 = _win32;
export const posix = _posix;
export const {
  basename,
  delimiter,
  dirname,
  extname,
  format,
  fromFileUrl,
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
  sep,
  toFileUrl,
  toNamespacedPath,
} = path;

export * from "internal:deno_node/polyfills/path/common.ts";
export {
  SEP,
  SEP_PATTERN,
} from "internal:deno_node/polyfills/path/separator.ts";
export * from "internal:deno_node/polyfills/path/_interface.ts";
export * from "internal:deno_node/polyfills/path/glob.ts";
