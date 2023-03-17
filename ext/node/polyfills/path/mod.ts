// Copyright the Browserify authors. MIT License.
// Ported mostly from https://github.com/browserify/path-browserify/
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { isWindows } from "ext:deno_node/_util/os.ts";
import _win32 from "ext:deno_node/path/win32.ts";
import _posix from "ext:deno_node/path/posix.ts";

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

export * from "ext:deno_node/path/common.ts";
export { SEP, SEP_PATTERN } from "ext:deno_node/path/separator.ts";
export * from "ext:deno_node/path/_interface.ts";
export * from "ext:deno_node/path/glob.ts";
