// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/path/mod.ts");
export const {
  win32,
  posix,
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
  common,
} = _mod;
export default _mod.default;
