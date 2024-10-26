// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core } from "ext:core/mod.js";
import {
  op_net_listen_udp,
  op_net_listen_unixpacket,
  op_runtime_memory_usage,
} from "ext:core/ops";

import * as timers from "ext:deno_web/02_timers.js";
import * as httpClient from "ext:deno_fetch/22_http_client.js";
import * as console from "ext:deno_console/01_console.js";
import * as ffi from "ext:deno_ffi/00_ffi.js";
import * as net from "ext:deno_net/01_net.js";
import * as tls from "ext:deno_net/02_tls.js";
import * as serve from "ext:deno_http/00_serve.ts";
import * as http from "ext:deno_http/01_http.js";
import * as websocket from "ext:deno_http/02_websocket.ts";
import * as errors from "ext:runtime/01_errors.js";
import * as version from "ext:runtime/01_version.ts";
import * as permissions from "ext:runtime/10_permissions.js";
import * as io from "ext:deno_io/12_io.js";
import * as fs from "ext:deno_fs/30_fs.js";
import * as os from "ext:runtime/30_os.js";
import * as fsEvents from "ext:runtime/40_fs_events.js";
import * as process from "ext:runtime/40_process.js";
import * as signals from "ext:runtime/40_signals.js";
import * as tty from "ext:runtime/40_tty.js";
import * as kv from "ext:deno_kv/01_db.ts";
import * as cron from "ext:deno_cron/01_cron.ts";
import * as webgpuSurface from "ext:deno_webgpu/02_surface.js";

const denoNs = {
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
  memoryUsage: () => op_runtime_memory_usage(),
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
  errors: errors.errors,
  inspect: console.inspect,
  env: os.env,
  exit: os.exit,
  execPath: os.execPath,
  SeekMode: io.SeekMode,
  FsFile: fs.FsFile,
  open: fs.open,
  openSync: fs.openSync,
  create: fs.create,
  createSync: fs.createSync,
  stdin: io.stdin,
  stdout: io.stdout,
  stderr: io.stderr,
  connect: net.connect,
  listen: net.listen,
  loadavg: os.loadavg,
  connectTls: tls.connectTls,
  listenTls: tls.listenTls,
  startTls: tls.startTls,
  symlink: fs.symlink,
  symlinkSync: fs.symlinkSync,
  link: fs.link,
  linkSync: fs.linkSync,
  permissions: permissions.permissions,
  Permissions: permissions.Permissions,
  PermissionStatus: permissions.PermissionStatus,
  serveHttp: http.serveHttp,
  serve: serve.serve,
  resolveDns: net.resolveDns,
  upgradeWebSocket: websocket.upgradeWebSocket,
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
  ChildProcess: process.ChildProcess,
  dlopen: ffi.dlopen,
  UnsafeCallback: ffi.UnsafeCallback,
  UnsafePointer: ffi.UnsafePointer,
  UnsafePointerView: ffi.UnsafePointerView,
  UnsafeFnPointer: ffi.UnsafeFnPointer,
  umask: fs.umask,
  HttpClient: httpClient.HttpClient,
  createHttpClient: httpClient.createHttpClient,
};

// NOTE(bartlomieju): keep IDs in sync with `cli/main.rs`
const unstableIds = {
  broadcastChannel: 1,
  cron: 2,
  ffi: 3,
  fs: 4,
  http: 5,
  kv: 6,
  net: 7,
  process: 8,
  temporal: 9,
  unsafeProto: 10,
  webgpu: 11,
  workerOptions: 12,
};

const denoNsUnstableById = { __proto__: null };

// denoNsUnstableById[unstableIds.broadcastChannel] = { __proto__: null }

denoNsUnstableById[unstableIds.cron] = {
  cron: cron.cron,
};

denoNsUnstableById[unstableIds.kv] = {
  openKv: kv.openKv,
  AtomicOperation: kv.AtomicOperation,
  Kv: kv.Kv,
  KvU64: kv.KvU64,
  KvListIterator: kv.KvListIterator,
};

denoNsUnstableById[unstableIds.net] = {
  listenDatagram: net.createListenDatagram(
    op_net_listen_udp,
    op_net_listen_unixpacket,
  ),
};

// denoNsUnstableById[unstableIds.unsafeProto] = { __proto__: null }

denoNsUnstableById[unstableIds.webgpu] = {
  UnsafeWindowSurface: webgpuSurface.UnsafeWindowSurface,
};

// denoNsUnstableById[unstableIds.workerOptions] = { __proto__: null }

export { denoNs, denoNsUnstableById, unstableIds };
