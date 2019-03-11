// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Public deno module.
export { noColor, pid, env, exit, isTTY, execPath } from "./os";
export { chdir, cwd } from "./dir";
export {
  File,
  open,
  stdin,
  stdout,
  stderr,
  read,
  write,
  seek,
  close,
  OpenMode
} from "./files";
export {
  copy,
  toAsyncIterator,
  ReadResult,
  SeekMode,
  Reader,
  Writer,
  Closer,
  Seeker,
  ReadCloser,
  WriteCloser,
  ReadSeeker,
  WriteSeeker,
  ReadWriteCloser,
  ReadWriteSeeker
} from "./io";
export { Buffer, readAll } from "./buffer";
export { mkdirSync, mkdir } from "./mkdir";
export {
  makeTempDirSync,
  makeTempDir,
  MakeTempDirOptions
} from "./make_temp_dir";
export { chmodSync, chmod } from "./chmod";
export { removeSync, remove, RemoveOption } from "./remove";
export { renameSync, rename } from "./rename";
export { readFileSync, readFile } from "./read_file";
export { readDirSync, readDir } from "./read_dir";
export { copyFileSync, copyFile } from "./copy_file";
export { readlinkSync, readlink } from "./read_link";
export { statSync, lstatSync, stat, lstat } from "./stat";
export { symlinkSync, symlink } from "./symlink";
export { writeFileSync, writeFile, WriteFileOptions } from "./write_file";
export { ErrorKind, DenoError } from "./errors";
export { libdeno } from "./libdeno";
export {
  permissions,
  revokePermission,
  Permission,
  Permissions
} from "./permissions";
export { truncateSync, truncate } from "./truncate";
export { FileInfo } from "./file_info";
export { connect, dial, listen, Listener, Conn } from "./net";
export { metrics, Metrics } from "./metrics";
export { resources } from "./resources";
export { run, RunOptions, Process, ProcessStatus } from "./process";
export { inspect } from "./console";
export { build, platform, OperatingSystem } from "./build";
export { version } from "./version";
export const args: string[] = [];

// TODO Don't expose Console nor stringifyArgs.
/** @internal */
export { Console, stringifyArgs } from "./console";
// TODO Don't expose DomIterableMixin.
/** @internal */
export { DomIterableMixin } from "./mixins/dom_iterable";
