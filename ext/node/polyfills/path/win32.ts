// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/path/mod.ts");

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
  matchesGlob,
} = _mod.win32;

export const posix = _mod.posix;
export const win32 = _mod.win32;
export default _mod.win32;
