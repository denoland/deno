// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/path/mod.ts");

export const {
  basename,
  delimiter,
  dirname,
  extname,
  format,
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
  sep,
  toNamespacedPath,
  _makeLong,
} = mod.win32;

export const posix = mod.posix;
export const win32 = mod.win32;
export default mod.win32;
