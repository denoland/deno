// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports unstable Deno APIs.

export { umask } from "./ops/fs/umask.ts";
export { linkSync, link } from "./ops/fs/link.ts";
export { symlinkSync, symlink } from "./ops/fs/symlink.ts";
export { dir, loadavg, osRelease, hostname } from "./ops/os.ts";
export { openPlugin } from "./ops/plugins.ts";
export { transpileOnly, compile, bundle } from "./compiler_api.ts";
export { applySourceMap, formatDiagnostics } from "./ops/errors.ts";
export { signal, signals, Signal, SignalStream } from "./signals.ts";
export { setRaw } from "./ops/tty.ts";
export { utimeSync, utime } from "./ops/fs/utime.ts";
export { ShutdownMode, shutdown } from "./net.ts";
export { listen, listenDatagram, connect } from "./net_unstable.ts";
export { startTls } from "./tls.ts";
export { kill } from "./ops/process.ts";
export {
  permissions,
  PermissionName,
  PermissionState,
  PermissionStatus,
  Permissions,
} from "./permissions.ts";
export {
  Diagnostic,
  DiagnosticCategory,
  DiagnosticItem,
  DiagnosticMessageChain,
} from "./diagnostics.ts";
