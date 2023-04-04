// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
import * as timers from "ext:deno_web/02_timers.js";
import * as httpClient from "ext:deno_fetch/22_http_client.js";
import * as console from "ext:deno_console/02_console.js";
import * as ffi from "ext:deno_ffi/00_ffi.js";
import * as net from "ext:deno_net/01_net.js";
import * as tls from "ext:deno_net/02_tls.js";
import * as http from "ext:deno_http/01_http.js";
import * as errors from "ext:runtime/01_errors.js";
import * as version from "ext:runtime/01_version.ts";
import * as permissions from "ext:runtime/10_permissions.js";
import * as io from "ext:deno_io/12_io.js";
import * as buffer from "ext:runtime/13_buffer.js";
import * as fs from "ext:deno_fs/30_fs.js";
import * as os from "ext:runtime/30_os.js";
import * as fsEvents from "ext:runtime/40_fs_events.js";
import * as process from "ext:runtime/40_process.js";
import * as signals from "ext:runtime/40_signals.js";
import * as tty from "ext:runtime/40_tty.js";
// TODO(bartlomieju): this is funky we have two `http` imports
import * as httpRuntime from "ext:runtime/40_http.js";
import * as kv from "ext:deno_kv/01_db.ts";

const denoNs = {
  metrics: core.metrics,
  Process: process.Process,
  run: process.run,
  isatty: tty.isatty,
  writeFileSync: fs.writeFileSync,
  writeFile: fs.writeFile,
  writeTextFileSync: fs.writeTextFileSync,
  writeTextFile: fs.writeTextFile,
  readTextFile: fs.readTextFile,
  readTextFileSync: fs.readTextFileSync,
  readFile: fs.readFile,
  readFileSync: fs.readFileSync,
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
  build: core.build,
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
  File: fs.File,
  FsFile: fs.FsFile,
  open: fs.open,
  openSync: fs.openSync,
  create: fs.create,
  createSync: fs.createSync,
  stdin: io.stdin,
  stdout: io.stdout,
  stderr: io.stderr,
  seek: fs.seek,
  seekSync: fs.seekSync,
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
  Command: process.Command,
  // TODO(bartlomieju): why is this exported?
  ChildProcess: process.ChildProcess,
};

const denoNsUnstable = {
  listenDatagram: net.createListenDatagram(
    ops.op_net_listen_udp,
    ops.op_net_listen_unixpacket,
  ),
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
  upgradeHttp: http.upgradeHttp,
  serve: http.serve,
  openKv: kv.openKv,
  Kv: kv.Kv,
  KvU64: kv.KvU64,
  KvListIterator: kv.KvListIterator,
};

export { denoNs, denoNsUnstable };
