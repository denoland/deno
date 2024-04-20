// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import path from "ext:deno_node/path/mod.ts";

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
} = path.posix;

export default path.posix;
