// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
import * as timers from "internal:deno_web/02_timers.js";
import * as httpClient from "internal:deno_fetch/22_http_client.js";
import * as console from "internal:deno_console/02_console.js";
import * as ffi from "internal:deno_ffi/00_ffi.js";
import * as net from "internal:deno_net/01_net.js";
import * as tls from "internal:deno_net/02_tls.js";
import * as http from "internal:deno_http/01_http.js";
import * as flash from "internal:deno_flash/01_http.js";
import * as build from "internal:runtime/js/01_build.js";
import * as errors from "internal:runtime/js/01_errors.js";
import * as version from "internal:runtime/js/01_version.ts";
import * as permissions from "internal:runtime/js/10_permissions.js";
import * as io from "internal:runtime/js/12_io.js";
import * as buffer from "internal:runtime/js/13_buffer.js";
import * as fs from "internal:runtime/js/30_fs.js";
import * as os from "internal:runtime/js/30_os.js";
import * as diagnostics from "internal:runtime/js/40_diagnostics.js";
import * as files from "internal:runtime/js/40_files.js";
import * as fsEvents from "internal:runtime/js/40_fs_events.js";
import * as process from "internal:runtime/js/40_process.js";
import * as readFile from "internal:runtime/js/40_read_file.js";
import * as signals from "internal:runtime/js/40_signals.js";
import * as tty from "internal:runtime/js/40_tty.js";
import * as writeFile from "internal:runtime/js/40_write_file.js";
import * as spawn from "internal:runtime/js/40_spawn.js";
// TODO(bartlomieju): this is funky we have two `http` imports
import * as httpRuntime from "internal:runtime/js/40_http.js";

const denoNs = {
  metrics: core.metrics,
  Process: process.Process,
  run: process.run,
  isatty: tty.isatty,
  writeFileSync: writeFile.writeFileSync,
  writeFile: writeFile.writeFile,
  writeTextFileSync: writeFile.writeTextFileSync,
  writeTextFile: writeFile.writeTextFile,
  readTextFile: readFile.readTextFile,
  readTextFileSync: readFile.readTextFileSync,
  readFile: readFile.readFile,
  readFileSync: readFile.readFileSync,
  watchFs: fsEvents.watchFs,
  chmodSync: fs.chmodSync,
  chmod: fs.chmod,
  chown: fs.chown,
  chownSync: fs.chownSync,
  copyFileSync: fs.copyFileSync,
  cwd: fs.cwd,
  makeTempDirSync: fs.makeTempDirSync,
  makeTempDir: fs.makeTempDir,
  makeTempFileSync: fs.makeTempFileSync,
  makeTempFile: fs.makeTempFile,
  memoryUsage: () => ops.op_runtime_memory_usage(),
  mkdirSync: fs.mkdirSync,
  mkdir: fs.mkdir,
  chdir: fs.chdir,
  copyFile: fs.copyFile,
  readDirSync: fs.readDirSync,
  readDir: fs.readDir,
  readLinkSync: fs.readLinkSync,
  readLink: fs.readLink,
  realPathSync: fs.realPathSync,
  realPath: fs.realPath,
  removeSync: fs.removeSync,
  remove: fs.remove,
  renameSync: fs.renameSync,
  rename: fs.rename,
  version: version.version,
  build: build.build,
  statSync: fs.statSync,
  lstatSync: fs.lstatSync,
  stat: fs.stat,
  lstat: fs.lstat,
  truncateSync: fs.truncateSync,
  truncate: fs.truncate,
  ftruncateSync: fs.ftruncateSync,
  ftruncate: fs.ftruncate,
  futime: fs.futime,
  futimeSync: fs.futimeSync,
  errors: errors.errors,
  // TODO(kt3k): Remove this export at v2
  // See https://github.com/denoland/deno/issues/9294
  customInspect: console.customInspect,
  inspect: console.inspect,
  env: os.env,
  exit: os.exit,
  execPath: os.execPath,
  Buffer: buffer.Buffer,
  readAll: buffer.readAll,
  readAllSync: buffer.readAllSync,
  writeAll: buffer.writeAll,
  writeAllSync: buffer.writeAllSync,
  copy: io.copy,
  iter: io.iter,
  iterSync: io.iterSync,
  SeekMode: io.SeekMode,
  read: io.read,
  readSync: io.readSync,
  write: io.write,
  writeSync: io.writeSync,
  File: files.File,
  FsFile: files.FsFile,
  open: files.open,
  openSync: files.openSync,
  create: files.create,
  createSync: files.createSync,
  stdin: files.stdin,
  stdout: files.stdout,
  stderr: files.stderr,
  seek: files.seek,
  seekSync: files.seekSync,
  connect: net.connect,
  listen: net.listen,
  loadavg: os.loadavg,
  connectTls: tls.connectTls,
  listenTls: tls.listenTls,
  startTls: tls.startTls,
  shutdown: net.shutdown,
  fstatSync: fs.fstatSync,
  fstat: fs.fstat,
  fsyncSync: fs.fsyncSync,
  fsync: fs.fsync,
  fdatasyncSync: fs.fdatasyncSync,
  fdatasync: fs.fdatasync,
  symlink: fs.symlink,
  symlinkSync: fs.symlinkSync,
  link: fs.link,
  linkSync: fs.linkSync,
  permissions: permissions.permissions,
  Permissions: permissions.Permissions,
  PermissionStatus: permissions.PermissionStatus,
  // TODO(bartlomieju): why is this not in one of extensions?
  serveHttp: httpRuntime.serveHttp,
  resolveDns: net.resolveDns,
  upgradeWebSocket: http.upgradeWebSocket,
  utime: fs.utime,
  utimeSync: fs.utimeSync,
  kill: process.kill,
  addSignalListener: signals.addSignalListener,
  removeSignalListener: signals.removeSignalListener,
  refTimer: timers.refTimer,
  unrefTimer: timers.unrefTimer,
  osRelease: os.osRelease,
  osUptime: os.osUptime,
  hostname: os.hostname,
  systemMemoryInfo: os.systemMemoryInfo,
  networkInterfaces: os.networkInterfaces,
  consoleSize: tty.consoleSize,
  gid: os.gid,
  uid: os.uid,
};

const denoNsUnstable = {
  DiagnosticCategory: diagnostics.DiagnosticCategory,
  listenDatagram: net.listenDatagram,
  umask: fs.umask,
  HttpClient: httpClient.HttpClient,
  createHttpClient: httpClient.createHttpClient,
  // TODO(bartlomieju): why is it needed?
  http,
  dlopen: ffi.dlopen,
  UnsafeCallback: ffi.UnsafeCallback,
  UnsafePointer: ffi.UnsafePointer,
  UnsafePointerView: ffi.UnsafePointerView,
  UnsafeFnPointer: ffi.UnsafeFnPointer,
  flock: fs.flock,
  flockSync: fs.flockSync,
  funlock: fs.funlock,
  funlockSync: fs.funlockSync,
  Child: spawn.Child,
  ChildProcess: spawn.ChildProcess,
  Command: spawn.Command,
  upgradeHttp: http.upgradeHttp,
  upgradeHttpRaw: flash.upgradeHttpRaw,
};

export { denoNs, denoNsUnstable };
