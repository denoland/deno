// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Public deno module.
export { dir, env, exit, isTTY, execPath, hostname } from "./os.ts";
export { chdir, cwd } from "./dir.ts";
export {
  File,
  open,
  openSync,
  create,
  createSync,
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
  OpenOptions,
  OpenMode
} from "./files.ts";
export {
  EOF,
  copy,
  toAsyncIterator,
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
} from "./io.ts";
export {
  Buffer,
  readAll,
  readAllSync,
  writeAll,
  writeAllSync
} from "./buffer.ts";
export { mkdirSync, mkdir } from "./mkdir.ts";
export {
  makeTempDirSync,
  makeTempDir,
  MakeTempDirOptions
} from "./make_temp_dir.ts";
export { chmodSync, chmod } from "./chmod.ts";
export { chownSync, chown } from "./chown.ts";
export { utimeSync, utime } from "./utime.ts";
export { removeSync, remove, RemoveOption } from "./remove.ts";
export { renameSync, rename } from "./rename.ts";
export { realpathSync, realpath } from "./realpath.ts";
export { readFileSync, readFile } from "./read_file.ts";
export { readDirSync, readDir } from "./read_dir.ts";
export { copyFileSync, copyFile } from "./copy_file.ts";
export { readlinkSync, readlink } from "./read_link.ts";
export { statSync, lstatSync, stat, lstat } from "./stat.ts";
export { linkSync, link } from "./link.ts";
export { symlinkSync, symlink } from "./symlink.ts";
export { writeFileSync, writeFile, WriteFileOptions } from "./write_file.ts";
export { applySourceMap } from "./error_stack.ts";
export { ErrorKind, DenoError } from "./errors.ts";
export {
  permissions,
  PermissionName,
  PermissionState,
  PermissionStatus,
  Permissions
} from "./permissions.ts";
export { truncateSync, truncate } from "./truncate.ts";
export { FileInfo } from "./file_info.ts";
export { openPlugin } from "./plugins.ts";
export {
  connect,
  listen,
  Listener,
  Conn,
  ShutdownMode,
  shutdown
} from "./net.ts";
export { connectTLS, listenTLS } from "./tls.ts";
export { metrics, Metrics } from "./metrics.ts";
export { resources } from "./resources.ts";
export {
  kill,
  run,
  RunOptions,
  Process,
  ProcessStatus,
  Signal
} from "./process.ts";
export { transpileOnly, compile, bundle } from "./compiler_api.ts";
export { inspect } from "./console.ts";
export { signal, signals, SignalStream } from "./signals.ts";
export { build, OperatingSystem, Arch } from "./build.ts";
export { version } from "./version.ts";
export const args: string[] = [];

// These are internal Deno APIs.  We are marking them as internal so they do not
// appear in the runtime type library.
/** @internal */
export { core } from "./core.ts";

/** The current process id of the runtime. */
export let pid: number;

/** Reflects the NO_COLOR environment variable: https://no-color.org/ */
export let noColor: boolean;

export { symbols } from "./symbols.ts";
