// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Public deno module.
export { env, exit, isTTY, execPath, homeDir } from "./os.ts";
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
export { readFileSync, readFile } from "./read_file.ts";
export {
  copyFileSync,
  copyFile,
  cwd,
  chdir,
  mkdir,
  mkdirSync,
  chmodSync,
  chmod,
  chownSync,
  chown,
  utimeSync,
  utime,
  removeSync,
  remove,
  RemoveOption,
  renameSync,
  rename,
  statSync,
  lstatSync,
  stat,
  lstat,
  readDirSync,
  readDir,
  readlinkSync,
  readlink,
  linkSync,
  link,
  symlinkSync,
  symlink,
  truncateSync,
  truncate,
  makeTempDirSync,
  makeTempDir,
  MakeTempDirOptions,
  FileInfo
} from "deno_ops_fs";
export { writeFileSync, writeFile, WriteFileOptions } from "./write_file.ts";
export { applySourceMap } from "./error_stack.ts";
export {
  permissions,
  revokePermission,
  Permission,
  Permissions
} from "./permissions.ts";
export { connect, dial, listen, Listener, Conn } from "./net.ts";
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
export { inspect, customInspect } from "./console.ts";
export {
  StandardErrorKinds,
  ErrorKind,
  DenoError,
  build,
  OperatingSystem,
  Arch
} from "deno_util";
export { version } from "./version.ts";
export const args: string[] = [];

// These are internal Deno APIs.  We are marking them as internal so they do not
// appear in the runtime type library.
/** @internal */
export { core, ops } from "deno_util";

/** @internal */
export { setPrepareStackTrace } from "./error_stack.ts";

// TODO Don't expose Console nor stringifyArgs.
/** @internal */
export { Console, stringifyArgs } from "./console.ts";
// TODO Don't expose DomIterableMixin.
/** @internal */
export { DomIterableMixin } from "./mixins/dom_iterable.ts";

/** The current process id of the runtime. */
export let pid: number;

/** Reflects the NO_COLOR environment variable: https://no-color.org/ */
export let noColor: boolean;

// TODO(ry) This should not be exposed to Deno.
export function _setGlobals(pid_: number, noColor_: boolean): void {
  pid = pid_;
  noColor = noColor_;
}
