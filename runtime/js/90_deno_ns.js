// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_net_listen_udp,
  op_net_listen_unixpacket,
  op_runtime_cpu_usage,
  op_runtime_memory_usage,
} from "ext:core/ops";

// Eagerly loaded modules. These are required by the bootstrap path itself
// (99_main.js loads errors/version/telemetry; console is referenced by
// 98_global_scope_shared.js for the global console singleton; surface is tiny).
// kv and cron are .ts files that the cli runtime can't transpile post-bootstrap,
// so they have to load at snapshot time.
const console = core.loadExtScript("ext:deno_web/01_console.js");
const errors = core.loadExtScript("ext:runtime/01_errors.js");
const version = core.loadExtScript("ext:runtime/01_version.ts");
const surface = core.loadExtScript("ext:deno_canvas/02_surface.js");
const telemetry = core.loadExtScript("ext:deno_telemetry/telemetry.ts");
const kv = core.loadExtScript("ext:deno_kv/01_db.ts");
const cron = core.loadExtScript("ext:deno_cron/01_cron.ts");
import { unstableIds } from "ext:deno_features/flags.js";
const { loadWebGPU } = core.loadExtScript("ext:deno_webgpu/00_init.js");
import { bundle } from "ext:deno_bundle_runtime/bundle.ts";

// Lazy module loaders. Each is a memoizing function (loadExtScript caches the
// result), so the first access to any property derived from these modules
// triggers a single module load. At idle, none of these modules are evaluated.
const loadTimers = () => core.loadExtScript("ext:deno_web/02_timers.js");
const loadFfi = () => core.loadExtScript("ext:deno_ffi/00_ffi.js");
const loadHttpClient = () =>
  core.loadExtScript("ext:deno_fetch/22_http_client.js");
const loadNet = () => core.loadExtScript("ext:deno_net/01_net.js");
const loadTls = () => core.loadExtScript("ext:deno_net/02_tls.js");
const loadServe = () => core.loadExtScript("ext:deno_http/00_serve.ts");
const loadHttp = () => core.loadExtScript("ext:deno_http/01_http.js");
const loadHttpWebsocket = () =>
  core.loadExtScript("ext:deno_http/02_websocket.ts");
const loadPermissions = () =>
  core.loadExtScript("ext:runtime/10_permissions.js");
const loadIo = () => core.loadExtScript("ext:deno_io/12_io.js");
const loadFs = () => core.loadExtScript("ext:deno_fs/30_fs.js");
const loadOs = () => core.loadExtScript("ext:deno_os/30_os.js");
const loadFsEvents = () => core.loadExtScript("ext:runtime/40_fs_events.js");
const loadProcess = () => core.loadExtScript("ext:deno_process/40_process.js");
const loadSignals = () => core.loadExtScript("ext:deno_os/40_signals.js");
const loadTty = () => core.loadExtScript("ext:runtime/40_tty.js");

const { ObjectDefineProperties, Float64Array } = primordials;

const loadQuic = core.createLazyLoader("ext:deno_net/03_quic.js");
const loadWebTransport = core.createLazyLoader(
  "ext:deno_web/webtransport.js",
);

// the out buffer for `cpuUsage` and `memoryUsage`
const usageBuffer = new Float64Array(4);

// `denoNs` only contains entries that don't pull in another module. Everything
// that's just a reference to a value from a lazy-loadable module lives in
// `denoNsLazy` so 99_main.js's `...denoNs` spread doesn't force eager loads.
const denoNs = {
  cpuUsage: () => {
    op_runtime_cpu_usage(usageBuffer);
    const { 0: system, 1: user } = usageBuffer;
    return {
      system,
      user,
    };
  },
  memoryUsage: () => {
    op_runtime_memory_usage(usageBuffer);
    const {
      0: rss,
      1: heapTotal,
      2: heapUsed,
      3: external,
    } = usageBuffer;
    return {
      rss,
      heapTotal,
      heapUsed,
      external,
    };
  },
  version: version.version,
  build: core.build,
  errors: errors.errors,
  inspect: console.inspect,
  telemetry: telemetry.telemetry,
};

// Lazy property descriptors. Applied on the final Deno namespace via
// ObjectDefineProperties so the spread of `denoNs` doesn't fire the getters.
const denoNsLazy = {
  // ffi (deno_ffi/00_ffi.js)
  dlopen: core.propWritableLazyLoaded((m) => m.dlopen, loadFfi),
  UnsafeCallback: core.propWritableLazyLoaded(
    (m) => m.UnsafeCallback,
    loadFfi,
  ),
  UnsafePointer: core.propWritableLazyLoaded((m) => m.UnsafePointer, loadFfi),
  UnsafePointerView: core.propWritableLazyLoaded(
    (m) => m.UnsafePointerView,
    loadFfi,
  ),
  UnsafeFnPointer: core.propWritableLazyLoaded(
    (m) => m.UnsafeFnPointer,
    loadFfi,
  ),
  // httpClient (deno_fetch/22_http_client.js)
  HttpClient: core.propWritableLazyLoaded((m) => m.HttpClient, loadHttpClient),
  createHttpClient: core.propWritableLazyLoaded(
    (m) => m.createHttpClient,
    loadHttpClient,
  ),
  // process (deno_process/40_process.js)
  Process: core.propWritableLazyLoaded((m) => m.Process, loadProcess),
  run: core.propWritableLazyLoaded((m) => m.run, loadProcess),
  Command: core.propWritableLazyLoaded((m) => m.Command, loadProcess),
  ChildProcess: core.propWritableLazyLoaded((m) => m.ChildProcess, loadProcess),
  spawn: core.propWritableLazyLoaded((m) => m.spawn, loadProcess),
  spawnAndWait: core.propWritableLazyLoaded((m) => m.spawnAndWait, loadProcess),
  spawnAndWaitSync: core.propWritableLazyLoaded(
    (m) => m.spawnAndWaitSync,
    loadProcess,
  ),
  kill: core.propWritableLazyLoaded((m) => m.kill, loadProcess),
  // tty (runtime/40_tty.js)
  isatty: core.propWritableLazyLoaded((m) => m.isatty, loadTty),
  consoleSize: core.propWritableLazyLoaded((m) => m.consoleSize, loadTty),
  // fs (deno_fs/30_fs.js)
  writeFileSync: core.propWritableLazyLoaded((m) => m.writeFileSync, loadFs),
  writeFile: core.propWritableLazyLoaded((m) => m.writeFile, loadFs),
  writeTextFileSync: core.propWritableLazyLoaded(
    (m) => m.writeTextFileSync,
    loadFs,
  ),
  writeTextFile: core.propWritableLazyLoaded((m) => m.writeTextFile, loadFs),
  readTextFile: core.propWritableLazyLoaded((m) => m.readTextFile, loadFs),
  readTextFileSync: core.propWritableLazyLoaded(
    (m) => m.readTextFileSync,
    loadFs,
  ),
  readFile: core.propWritableLazyLoaded((m) => m.readFile, loadFs),
  readFileSync: core.propWritableLazyLoaded((m) => m.readFileSync, loadFs),
  chmodSync: core.propWritableLazyLoaded((m) => m.chmodSync, loadFs),
  chmod: core.propWritableLazyLoaded((m) => m.chmod, loadFs),
  chown: core.propWritableLazyLoaded((m) => m.chown, loadFs),
  chownSync: core.propWritableLazyLoaded((m) => m.chownSync, loadFs),
  copyFileSync: core.propWritableLazyLoaded((m) => m.copyFileSync, loadFs),
  cwd: core.propWritableLazyLoaded((m) => m.cwd, loadFs),
  makeTempDirSync: core.propWritableLazyLoaded(
    (m) => m.makeTempDirSync,
    loadFs,
  ),
  makeTempDir: core.propWritableLazyLoaded((m) => m.makeTempDir, loadFs),
  makeTempFileSync: core.propWritableLazyLoaded(
    (m) => m.makeTempFileSync,
    loadFs,
  ),
  makeTempFile: core.propWritableLazyLoaded((m) => m.makeTempFile, loadFs),
  mkdirSync: core.propWritableLazyLoaded((m) => m.mkdirSync, loadFs),
  mkdir: core.propWritableLazyLoaded((m) => m.mkdir, loadFs),
  chdir: core.propWritableLazyLoaded((m) => m.chdir, loadFs),
  copyFile: core.propWritableLazyLoaded((m) => m.copyFile, loadFs),
  readDirSync: core.propWritableLazyLoaded((m) => m.readDirSync, loadFs),
  readDir: core.propWritableLazyLoaded((m) => m.readDir, loadFs),
  readLinkSync: core.propWritableLazyLoaded((m) => m.readLinkSync, loadFs),
  readLink: core.propWritableLazyLoaded((m) => m.readLink, loadFs),
  realPathSync: core.propWritableLazyLoaded((m) => m.realPathSync, loadFs),
  realPath: core.propWritableLazyLoaded((m) => m.realPath, loadFs),
  removeSync: core.propWritableLazyLoaded((m) => m.removeSync, loadFs),
  remove: core.propWritableLazyLoaded((m) => m.remove, loadFs),
  renameSync: core.propWritableLazyLoaded((m) => m.renameSync, loadFs),
  rename: core.propWritableLazyLoaded((m) => m.rename, loadFs),
  statSync: core.propWritableLazyLoaded((m) => m.statSync, loadFs),
  lstatSync: core.propWritableLazyLoaded((m) => m.lstatSync, loadFs),
  stat: core.propWritableLazyLoaded((m) => m.stat, loadFs),
  lstat: core.propWritableLazyLoaded((m) => m.lstat, loadFs),
  truncateSync: core.propWritableLazyLoaded((m) => m.truncateSync, loadFs),
  truncate: core.propWritableLazyLoaded((m) => m.truncate, loadFs),
  symlink: core.propWritableLazyLoaded((m) => m.symlink, loadFs),
  symlinkSync: core.propWritableLazyLoaded((m) => m.symlinkSync, loadFs),
  link: core.propWritableLazyLoaded((m) => m.link, loadFs),
  linkSync: core.propWritableLazyLoaded((m) => m.linkSync, loadFs),
  utime: core.propWritableLazyLoaded((m) => m.utime, loadFs),
  utimeSync: core.propWritableLazyLoaded((m) => m.utimeSync, loadFs),
  umask: core.propWritableLazyLoaded((m) => m.umask, loadFs),
  open: core.propWritableLazyLoaded((m) => m.open, loadFs),
  openSync: core.propWritableLazyLoaded((m) => m.openSync, loadFs),
  create: core.propWritableLazyLoaded((m) => m.create, loadFs),
  createSync: core.propWritableLazyLoaded((m) => m.createSync, loadFs),
  FsFile: core.propWritableLazyLoaded((m) => m.FsFile, loadFs),
  // io (deno_io/12_io.js)
  SeekMode: core.propWritableLazyLoaded((m) => m.SeekMode, loadIo),
  stdin: core.propWritableLazyLoaded((m) => m.stdin, loadIo),
  stdout: core.propWritableLazyLoaded((m) => m.stdout, loadIo),
  stderr: core.propWritableLazyLoaded((m) => m.stderr, loadIo),
  // fsEvents (runtime/40_fs_events.js)
  watchFs: core.propWritableLazyLoaded((m) => m.watchFs, loadFsEvents),
  // os (deno_os/30_os.js)
  env: core.propWritableLazyLoaded((m) => m.env, loadOs),
  exit: core.propWritableLazyLoaded((m) => m.exit, loadOs),
  execPath: core.propWritableLazyLoaded((m) => m.execPath, loadOs),
  loadavg: core.propWritableLazyLoaded((m) => m.loadavg, loadOs),
  osRelease: core.propWritableLazyLoaded((m) => m.osRelease, loadOs),
  osUptime: core.propWritableLazyLoaded((m) => m.osUptime, loadOs),
  hostname: core.propWritableLazyLoaded((m) => m.hostname, loadOs),
  systemMemoryInfo: core.propWritableLazyLoaded(
    (m) => m.systemMemoryInfo,
    loadOs,
  ),
  networkInterfaces: core.propWritableLazyLoaded(
    (m) => m.networkInterfaces,
    loadOs,
  ),
  gid: core.propWritableLazyLoaded((m) => m.gid, loadOs),
  uid: core.propWritableLazyLoaded((m) => m.uid, loadOs),
  // signals (deno_os/40_signals.js)
  addSignalListener: core.propWritableLazyLoaded(
    (m) => m.addSignalListener,
    loadSignals,
  ),
  removeSignalListener: core.propWritableLazyLoaded(
    (m) => m.removeSignalListener,
    loadSignals,
  ),
  // permissions (runtime/10_permissions.js)
  permissions: core.propWritableLazyLoaded(
    (m) => m.permissions,
    loadPermissions,
  ),
  Permissions: core.propWritableLazyLoaded(
    (m) => m.Permissions,
    loadPermissions,
  ),
  PermissionStatus: core.propWritableLazyLoaded(
    (m) => m.PermissionStatus,
    loadPermissions,
  ),
  // net (deno_net/01_net.js)
  connect: core.propWritableLazyLoaded((m) => m.connect, loadNet),
  listen: core.propWritableLazyLoaded((m) => m.listen, loadNet),
  resolveDns: core.propWritableLazyLoaded((m) => m.resolveDns, loadNet),
  // tls (deno_net/02_tls.js)
  connectTls: core.propWritableLazyLoaded((m) => m.connectTls, loadTls),
  listenTls: core.propWritableLazyLoaded((m) => m.listenTls, loadTls),
  startTls: core.propWritableLazyLoaded((m) => m.startTls, loadTls),
  // http/serve/websocket
  serveHttp: core.propWritableLazyLoaded((m) => m.serveHttp, loadHttp),
  serve: core.propWritableLazyLoaded((m) => m.serve, loadServe),
  upgradeWebSocket: core.propWritableLazyLoaded(
    (m) => m.upgradeWebSocket,
    loadHttpWebsocket,
  ),
  // timers (deno_web/02_timers.js)
  refTimer: core.propWritableLazyLoaded((m) => m.refTimer, loadTimers),
  unrefTimer: core.propWritableLazyLoaded((m) => m.unrefTimer, loadTimers),
};

const denoNsUnstableById = { __proto__: null };

denoNsUnstableById[unstableIds.bundle] = {
  bundle,
};

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

denoNsUnstableById[unstableIds.net] = {};
let _listenDatagram;
ObjectDefineProperties(denoNsUnstableById[unstableIds.net], {
  listenDatagram: {
    __proto__: null,
    enumerable: true,
    configurable: true,
    get() {
      if (!_listenDatagram) {
        _listenDatagram = loadNet().createListenDatagram(
          op_net_listen_udp,
          op_net_listen_unixpacket,
        );
      }
      return _listenDatagram;
    },
    set(v) {
      _listenDatagram = v;
    },
  },
  connectQuic: core.propWritableLazyLoaded((q) => q.connectQuic, loadQuic),
  QuicEndpoint: core.propWritableLazyLoaded((q) => q.QuicEndpoint, loadQuic),
  QuicBidirectionalStream: core.propWritableLazyLoaded(
    (q) => q.QuicBidirectionalStream,
    loadQuic,
  ),
  QuicConn: core.propWritableLazyLoaded((q) => q.QuicConn, loadQuic),
  QuicListener: core.propWritableLazyLoaded((q) => q.QuicListener, loadQuic),
  QuicReceiveStream: core.propWritableLazyLoaded(
    (q) => q.QuicReceiveStream,
    loadQuic,
  ),
  QuicSendStream: core.propWritableLazyLoaded(
    (q) => q.QuicSendStream,
    loadQuic,
  ),
  QuicIncoming: core.propWritableLazyLoaded((q) => q.QuicIncoming, loadQuic),
  upgradeWebTransport: core.propWritableLazyLoaded(
    (wt) => wt.upgradeWebTransport,
    loadWebTransport,
  ),
});

// denoNsUnstableById[unstableIds.unsafeProto] = { __proto__: null }

denoNsUnstableById[unstableIds.webgpu] = {
  UnsafeWindowSurface: surface.UnsafeWindowSurface,
};
ObjectDefineProperties(denoNsUnstableById[unstableIds.webgpu], {
  webgpu: core.propWritableLazyLoaded(
    (webgpu) => webgpu.denoNsWebGPU,
    loadWebGPU,
  ),
});

// denoNsUnstableById[unstableIds.workerOptions] = { __proto__: null }

export { denoNs, denoNsLazy, denoNsUnstableById, unstableIds };
