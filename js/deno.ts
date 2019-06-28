// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Public deno module.
export { noColor, pid, env, exit, isTTY, execPath, homeDir } from "./os";
export { chdir, cwd } from "./dir";
export {
  File,
  open,
  openSync,
  stdin,
  stdout,
  stderr,
  read,
  readSync,
  write,
  writeSync,
  seek,
  seekSync,
  close,
  OpenMode
} from "./files";
export {
  copy,
  toAsyncIterator,
  ReadResult,
  SeekMode,
  Reader,
  SyncReader,
  Writer,
  SyncWriter,
  Closer,
  Seeker,
  SyncSeeker,
  ReadCloser,
  WriteCloser,
  ReadSeeker,
  WriteSeeker,
  ReadWriteCloser,
  ReadWriteSeeker
} from "./io";
export { Buffer, readAll, readAllSync } from "./buffer";
export { mkdirSync, mkdir } from "./mkdir";
export {
  makeTempDirSync,
  makeTempDir,
  MakeTempDirOptions
} from "./make_temp_dir";
export { chmodSync, chmod } from "./chmod";
export { chownSync, chown } from "./chown";
export { utimeSync, utime } from "./utime";
export { removeSync, remove, RemoveOption } from "./remove";
export { renameSync, rename } from "./rename";
export { readFileSync, readFile } from "./read_file";
export { readDirSync, readDir } from "./read_dir";
export { copyFileSync, copyFile } from "./copy_file";
export { readlinkSync, readlink } from "./read_link";
export { statSync, lstatSync, stat, lstat } from "./stat";
export { linkSync, link } from "./link";
export { symlinkSync, symlink } from "./symlink";
export { writeFileSync, writeFile, WriteFileOptions } from "./write_file";
export { ErrorKind, DenoError } from "./errors";
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
export {
  kill,
  run,
  RunOptions,
  Process,
  ProcessStatus,
  Signal
} from "./process";
export { inspect } from "./console";
export { build, platform, OperatingSystem, Arch } from "./build";
export { version } from "./version";
export const args: string[] = [];

// These are internal Deno APIs.  We are marking them as internal so they do not
// appear in the runtime type library.
/** @internal */
export { core } from "./core";

// TODO Don't expose Console nor stringifyArgs.
/** @internal */
export { Console, stringifyArgs } from "./console";
// TODO Don't expose DomIterableMixin.
/** @internal */
export { DomIterableMixin } from "./mixins/dom_iterable";
