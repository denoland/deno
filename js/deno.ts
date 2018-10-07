// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
/// <amd-module name="deno"/>
export { env, exit } from "./os";
export { File, open, stdin, stdout, stderr, read, write, close } from "./files";
export {
	copy, ReadResult, Reader, Writer, Closer, Seeker, ReaderCloser,
	WriteCloser, ReadSeeker, WriteSeeker, ReadWriteCloser, ReadWriteSeeker,
} from "./io";
export { mkdirSync, mkdir } from "./mkdir";
export { makeTempDirSync, makeTempDir } from "./make_temp_dir";
export { removeSync, remove, removeAllSync, removeAll } from "./remove";
export { renameSync, rename } from "./rename";
export { readFileSync, readFile } from "./read_file";
export { readDirSync, readDir } from "./read_dir";
export { copyFileSync, copyFile } from "./copy_file";
export { readlinkSync, readlink } from "./read_link";
export { statSync, lstatSync, stat, lstat } from "./stat";
export { symlinkSync, symlink } from "./symlink";
export { writeFileSync, writeFile } from "./write_file";
export { ErrorKind, DenoError } from "./errors";
export { libdeno } from "./libdeno";
export { platform } from "./platform";
export { trace } from "./trace";
export { truncateSync, truncate } from "./truncate";
export { FileInfo } from "./file_info";
export { connect, dial, listen, Listener, Conn } from "./net";
export const args: string[] = [];

// Provide the compiler API in an obfuscated way
import * as compiler from "./compiler";
// @internal
export const _compiler = compiler;
