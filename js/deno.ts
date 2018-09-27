// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
/// <amd-module name="deno"/>
export { env, exit } from "./os";
export { mkdirSync, mkdir } from "./mkdir";
export { makeTempDirSync, makeTempDir } from "./make_temp_dir";
export { removeSync, remove, removeAllSync, removeAll } from "./remove";
export { renameSync, rename } from "./rename";
export { readFileSync, readFile } from "./read_file";
export { readlinkSync, readlink } from "./read_link";
export { FileInfo, statSync, lstatSync, stat, lstat } from "./stat";
export { symlinkSync, symlink } from "./symlink";
export { writeFileSync, writeFile } from "./write_file";
export { ErrorKind, DenoError } from "./errors";
export { libdeno } from "./libdeno";
export { arch, platform } from "./platform";
export { trace } from "./trace";
export { setGlobalTimeout } from "./timers";
export const args: string[] = [];
