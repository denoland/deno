// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Public deno module.
export {
  Buffer,
  readAll,
  readAllSync,
  writeAll,
  writeAllSync,
} from "./buffer.ts";
export { build, OperatingSystem, Arch } from "./build.ts";
export { chmodSync, chmod } from "./ops/fs/chmod.ts";
export { chownSync, chown } from "./ops/fs/chown.ts";
export { transpileOnly, compile, bundle } from "./compiler/api.ts";
export { inspect } from "./web/console.ts";
export { copyFileSync, copyFile } from "./ops/fs/copy_file.ts";
export {
  Diagnostic,
  DiagnosticCategory,
  DiagnosticItem,
  DiagnosticMessageChain,
} from "./diagnostics.ts";
export { chdir, cwd } from "./ops/fs/dir.ts";
export { applySourceMap, formatDiagnostics } from "./ops/errors.ts";
export { errors } from "./errors.ts";
export { FileInfo } from "./file_info.ts";
export {
  File,
  open,
  openSync,
  create,
  createSync,
  stdin,
  stdout,
  stderr,
  seek,
  seekSync,
  OpenOptions,
  OpenMode,
} from "./files.ts";
export { read, readSync, write, writeSync } from "./ops/io.ts";
export { FsEvent, fsEvents } from "./ops/fs_events.ts";
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
  ReadWriteSeeker,
} from "./io.ts";
export { linkSync, link } from "./ops/fs/link.ts";
export {
  makeTempDirSync,
  makeTempDir,
  makeTempFileSync,
  makeTempFile,
  MakeTempOptions,
} from "./ops/fs/make_temp.ts";
export { metrics, Metrics } from "./ops/runtime.ts";
export { mkdirSync, mkdir, MkdirOptions } from "./ops/fs/mkdir.ts";
export {
  connect,
  listen,
  DatagramConn,
  Listener,
  Conn,
  ShutdownMode,
  shutdown,
} from "./net.ts";
export {
  dir,
  env,
  exit,
  execPath,
  hostname,
  loadavg,
  osRelease,
} from "./ops/os.ts";
export {
  permissions,
  PermissionName,
  PermissionState,
  PermissionStatus,
  Permissions,
} from "./permissions.ts";
export { openPlugin } from "./plugins.ts";
export { kill } from "./ops/process.ts";
export { run, RunOptions, Process, ProcessStatus } from "./process.ts";
export { readdirSync, readdir } from "./ops/fs/read_dir.ts";
export { readFileSync, readFile } from "./read_file.ts";
export { readlinkSync, readlink } from "./ops/fs/read_link.ts";
export { realpathSync, realpath } from "./ops/fs/realpath.ts";
export { removeSync, remove, RemoveOptions } from "./ops/fs/remove.ts";
export { renameSync, rename } from "./ops/fs/rename.ts";
export { resources, close } from "./ops/resources.ts";
export { signal, signals, Signal, SignalStream } from "./signals.ts";
export { statSync, lstatSync, stat, lstat } from "./ops/fs/stat.ts";
export { symlinkSync, symlink } from "./ops/fs/symlink.ts";
export { connectTLS, listenTLS } from "./tls.ts";
export { truncateSync, truncate } from "./ops/fs/truncate.ts";
export { isatty, setRaw } from "./ops/tty.ts";
export { umask } from "./ops/fs/umask.ts";
export { utimeSync, utime } from "./ops/fs/utime.ts";
export { version } from "./version.ts";
export { writeFileSync, writeFile, WriteFileOptions } from "./write_file.ts";
export const args: string[] = [];
export { test, runTests, TestEvent, ConsoleTestReporter } from "./testing.ts";

// These are internal Deno APIs.  We are marking them as internal so they do not
// appear in the runtime type library.
export { core } from "./core.ts";

export let pid: number;

export let noColor: boolean;

export { symbols } from "./symbols.ts";
