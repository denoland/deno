// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals } from "ext:core/mod.js";
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
import * as quic from "ext:deno_net/03_quic.js";
import * as serve from "ext:deno_http/00_serve.ts";
import * as http from "ext:deno_http/01_http.js";
import * as websocket from "ext:deno_http/02_websocket.ts";
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
import * as kv from "ext:deno_kv/01_db.ts";
import * as cron from "ext:deno_cron/01_cron.ts";
import * as webgpuSurface from "ext:deno_webgpu/02_surface.js";

const denoNs = {
  metrics: () => {
    internals.warnOnDeprecatedApi("Deno.metrics()", new Error().stack);
    return core.metrics();
  },
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
  ftruncateSync(rid, len) {
    internals.warnOnDeprecatedApi(
      "Deno.ftruncateSync()",
      new Error().stack,
      "Use `Deno.FsFile.truncateSync()` instead.",
    );
    return fs.ftruncateSync(rid, len);
  },
  ftruncate(rid, len) {
    internals.warnOnDeprecatedApi(
      "Deno.ftruncate()",
      new Error().stack,
      "Use `Deno.FsFile.truncate()` instead.",
    );
    return fs.ftruncate(rid, len);
  },
  async futime(rid, atime, mtime) {
    internals.warnOnDeprecatedApi(
      "Deno.futime()",
      new Error().stack,
      "Use `Deno.FsFile.utime()` instead.",
    );
    await fs.futime(rid, atime, mtime);
  },
  futimeSync(rid, atime, mtime) {
    internals.warnOnDeprecatedApi(
      "Deno.futimeSync()",
      new Error().stack,
      "Use `Deno.FsFile.utimeSync()` instead.",
    );
    fs.futimeSync(rid, atime, mtime);
  },
  errors: errors.errors,
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
  read(rid, buffer) {
    internals.warnOnDeprecatedApi(
      "Deno.read()",
      new Error().stack,
      "Use `reader.read()` instead.",
    );
    return io.read(rid, buffer);
  },
  readSync(rid, buffer) {
    internals.warnOnDeprecatedApi(
      "Deno.readSync()",
      new Error().stack,
      "Use `reader.readSync()` instead.",
    );
    return io.readSync(rid, buffer);
  },
  write(rid, data) {
    internals.warnOnDeprecatedApi(
      "Deno.write()",
      new Error().stack,
      "Use `writer.write()` instead.",
    );
    return io.write(rid, data);
  },
  writeSync(rid, data) {
    internals.warnOnDeprecatedApi(
      "Deno.writeSync()",
      new Error().stack,
      "Use `writer.writeSync()` instead.",
    );
    return io.writeSync(rid, data);
  },
  File: fs.File,
  FsFile: fs.FsFile,
  open: fs.open,
  openSync: fs.openSync,
  create: fs.create,
  createSync: fs.createSync,
  stdin: io.stdin,
  stdout: io.stdout,
  stderr: io.stderr,
  seek(rid, offset, whence) {
    internals.warnOnDeprecatedApi(
      "Deno.seek()",
      new Error().stack,
      "Use `file.seek()` instead.",
    );
    return fs.seek(rid, offset, whence);
  },
  seekSync(rid, offset, whence) {
    internals.warnOnDeprecatedApi(
      "Deno.seekSync()",
      new Error().stack,
      "Use `file.seekSync()` instead.",
    );
    return fs.seekSync(rid, offset, whence);
  },
  connect: net.connect,
  listen: net.listen,
  loadavg: os.loadavg,
  connectTls: tls.connectTls,
  listenTls: tls.listenTls,
  startTls: tls.startTls,
  shutdown(rid) {
    internals.warnOnDeprecatedApi(
      "Deno.shutdown()",
      new Error().stack,
      "Use `Deno.Conn.closeWrite()` instead.",
    );
    net.shutdown(rid);
  },
  fstatSync(rid) {
    internals.warnOnDeprecatedApi(
      "Deno.fstatSync()",
      new Error().stack,
      "Use `Deno.FsFile.statSync()` instead.",
    );
    return fs.fstatSync(rid);
  },
  fstat(rid) {
    internals.warnOnDeprecatedApi(
      "Deno.fstat()",
      new Error().stack,
      "Use `Deno.FsFile.stat()` instead.",
    );
    return fs.fstat(rid);
  },
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

denoNsUnstableById[unstableIds.ffi] = {
  dlopen: ffi.dlopen,
  UnsafeCallback: ffi.UnsafeCallback,
  UnsafePointer: ffi.UnsafePointer,
  UnsafePointerView: ffi.UnsafePointerView,
  UnsafeFnPointer: ffi.UnsafeFnPointer,
};

denoNsUnstableById[unstableIds.fs] = {
  flock: fs.flock,
  flockSync: fs.flockSync,
  funlock: fs.funlock,
  funlockSync: fs.funlockSync,
  umask: fs.umask,
};

denoNsUnstableById[unstableIds.http] = {
  HttpClient: httpClient.HttpClient,
  createHttpClient: httpClient.createHttpClient,
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

  connectQuic: quic.connectQuic,
  listenQuic: quic.listenQuic,
  QuicBidirectionalStream: quic.QuicBidirectionalStream,
  QuicConn: quic.QuicConn,
  QuicListener: quic.QuicListener,
  QuicReceiveStream: quic.QuicReceiveStream,
  QuicSendStream: quic.QuicSendStream,
  QuicIncoming: quic.QuicIncoming,
};

// denoNsUnstableById[unstableIds.unsafeProto] = { __proto__: null }

denoNsUnstableById[unstableIds.webgpu] = {
  UnsafeWindowSurface: webgpuSurface.UnsafeWindowSurface,
};

// denoNsUnstableById[unstableIds.workerOptions] = { __proto__: null }

// when editing this list, also update unstableDenoProps in cli/tsc/99_main_compiler.js
const denoNsUnstable = {
  listenDatagram: net.createListenDatagram(
    op_net_listen_udp,
    op_net_listen_unixpacket,
  ),
  umask: fs.umask,
  HttpClient: httpClient.HttpClient,
  createHttpClient: httpClient.createHttpClient,
  dlopen: ffi.dlopen,
  UnsafeCallback: ffi.UnsafeCallback,
  UnsafePointer: ffi.UnsafePointer,
  UnsafePointerView: ffi.UnsafePointerView,
  UnsafeFnPointer: ffi.UnsafeFnPointer,
  UnsafeWindowSurface: webgpuSurface.UnsafeWindowSurface,
  flock: fs.flock,
  flockSync: fs.flockSync,
  funlock: fs.funlock,
  funlockSync: fs.funlockSync,
  openKv: kv.openKv,
  AtomicOperation: kv.AtomicOperation,
  Kv: kv.Kv,
  KvU64: kv.KvU64,
  KvListIterator: kv.KvListIterator,
  cron: cron.cron,
  connectQuic: quic.connectQuic,
  listenQuic: quic.listenQuic,
  QuicBidirectionalStream: quic.QuicBidirectionalStream,
  QuicConn: quic.QuicConn,
  QuicListener: quic.QuicListener,
  QuicReceiveStream: quic.QuicReceiveStream,
  QuicSendStream: quic.QuicSendStream,
  QuicIncoming: quic.QuicIncoming,
};

export { denoNs, denoNsUnstable, denoNsUnstableById, unstableIds };
