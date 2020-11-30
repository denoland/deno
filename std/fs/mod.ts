// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { isWindows } from "../_util/os.ts";
import * as _win32 from "./path_win32.ts";
import * as _posix from "./path_posix.ts";
const path = isWindows ? _win32 : _posix;

export const win32 = _win32;
export const posix = _posix;

export * from "./empty_dir.ts";
export * from "./ensure_dir.ts";
export * from "./ensure_file.ts";
export * from "./ensure_link.ts";
export * from "./ensure_symlink.ts";
export * from "./exists.ts";
export * from "./expand_glob.ts";
export * from "./move.ts";
export * from "./copy.ts";
export * from "./walk.ts";
export * from "./eol.ts";
export const {
  resolvePath,
  relativePath,
} = path;
