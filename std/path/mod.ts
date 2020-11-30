// Copyright the Browserify authors. MIT License.
// Ported mostly from https://github.com/browserify/path-browserify/
/** This module is browser compatible. */

import { isWindows } from "../_util/os.ts";
import * as _posix from "./posix.ts";
import * as _win32 from "./win32.ts";

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

export * from "./_interface.ts";
export * from "./common.ts";
export * from "./glob.ts";
export { SEP, SEP_PATTERN } from "./separator.ts";
