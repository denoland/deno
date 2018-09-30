// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
/// <amd-module name="deno"/>
export { env, exit } from "./os";
export { File, open, stdin, stdout, stderr, read, write, close } from "./files";
export { copy, Reader, Writer } from "./io";
export { mkdirSync, mkdir } from "./mkdir";
export { makeTempDirSync, makeTempDir } from "./make_temp_dir";
export { removeSync, remove, removeAllSync, removeAll } from "./remove";
export { renameSync, rename } from "./rename";
export { readFileSync, readFile } from "./read_file";
export { copyFileSync, copyFile } from "./copy_file";
export { readlinkSync, readlink } from "./read_link";
export { FileInfo, statSync, lstatSync, stat, lstat } from "./stat";
export { symlinkSync, symlink } from "./symlink";
export { writeFileSync, writeFile } from "./write_file";
export { ErrorKind, DenoError } from "./errors";
export { libdeno } from "./libdeno";
export { arch, platform } from "./platform";
export { trace } from "./trace";
export { truncateSync, truncate } from "./truncate";
export { setGlobalTimeout } from "./timers";
export const args: string[] = [];
