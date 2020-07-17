// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports unstable Deno APIs.
((window) => {
  // export { openPlugin } from "./ops/plugins.ts";
  // export { transpileOnly, compile, bundle } from "./compiler_api.ts";
  // export { signal, signals, Signal, SignalStream } from "./signals.ts";
  // export { setRaw, consoleSize } from "./ops/tty.ts";
  // export { kill } from "./ops/process.ts";
  // export { permissions, Permissions } from "./permissions.ts";
  // export { PermissionStatus } from "./permissions.ts";

  window.Deno = {
    ...window.Deno,
    ...{
      DiagnosticCategory: window.__diagnostics.DiagnosticCategory,
      loadavg: window.__os.loadavg,
      hostname: window.__os.hostname,
      osRelease: window.__os.osRelease,
      applySourceMap: window.__errorStack.opApplySourceMap,
      formatDiagnostics: window.__errorStack.opFormatDiagnostics,
      shutdown: window.__net.shutdown,
      ShutdownMode: window.__net.ShutdownMode,
      listen: window.__netUnstable.listen,
      connect: window.__netUnstable.connect,
      listenDatagram: window.__netUnstable.listenDatagram,
      startTls: window.__tls.startTls,
      fstatSync: window.__fs.fstatSync,
      fstat: window.__fs.fstat,
      ftruncateSync: window.__fs.ftruncateSync,
      ftruncate: window.__fs.ftruncate,
      umask: window.__fs.umask,
      link: window.__fs.link,
      linkSync: window.__fs.linkSync,
      utime: window.__fs.utime,
      utimeSync: window.__fs.utimeSync,
      symlink: window.__fs.symlink,
      symlinkSync: window.__fs.symlinkSync,
      fdatasyncSync: window.__fs.fdatasyncSync,
      fdatasync: window.__fs.fdatasync,
      fsyncSync: window.__fs.fsyncSync,
      fsync: window.__fs.fsync,
    },
  };
})(this);
