// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports unstable Deno APIs.
((window) => {
  window.__bootstrap.denoNsUnstable = {
    transpileOnly: window.__bootstrap.compilerApi.transpileOnly,
    compile: window.__bootstrap.compilerApi.compile,
    bundle: window.__bootstrap.compilerApi.bundle,
    permissions: window.__bootstrap.permissions.permissions,
    Permissions: window.__bootstrap.permissions.Permissions,
    PermissionStatus: window.__bootstrap.permissions.PermissionStatus,
    DiagnosticCategory: window.__bootstrap.diagnostics.DiagnosticCategory,
    applySourceMap: window.__bootstrap.errorStack.opApplySourceMap,
    formatDiagnostics: window.__bootstrap.errorStack.opFormatDiagnostics,
    listen: window.__bootstrap.netUnstable.listen,
    connect: window.__bootstrap.netUnstable.connect,
    listenDatagram: window.__bootstrap.netUnstable.listenDatagram,
  };
})(this);
