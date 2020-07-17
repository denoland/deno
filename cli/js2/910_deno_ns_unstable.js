// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports unstable Deno APIs.
((window) => {
  // export { umask } from "./ops/fs/umask.ts";
  // export { linkSync, link } from "./ops/fs/link.ts";
  // export { fstatSync, fstat } from "./ops/fs/stat.ts";
  // export { fdatasyncSync, fdatasync, fsyncSync, fsync } from "./ops/fs/sync.ts";
  // export { symlinkSync, symlink } from "./ops/fs/symlink.ts";
  // export { openPlugin } from "./ops/plugins.ts";
  // export { transpileOnly, compile, bundle } from "./compiler_api.ts";
  // export { signal, signals, Signal, SignalStream } from "./signals.ts";
  // export { setRaw, consoleSize } from "./ops/tty.ts";
  // export { utimeSync, utime } from "./ops/fs/utime.ts";
  // export { ftruncateSync, ftruncate } from "./ops/fs/truncate.ts";
  // export { shutdown, ShutdownMode } from "./net.ts";
  // export { listen, listenDatagram, connect } from "./net_unstable.ts";
  // export { startTls } from "./tls.ts";
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
    },
  };
})(this);
