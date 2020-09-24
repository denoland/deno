// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { fileName } from "../path/mod.ts";

import {
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
} from "../path/mod.ts";

function basename(path: string, ext?: string): string {
  const name = fileName(path) ?? "";
  if (ext != null && name.endsWith(ext)) {
    return name.slice(0, name.length - ext.length);
  }
  return name;
}

export {
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
};

export default {
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
};
