// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

import { core, internals } from "ext:core/mod.js";
const requireImpl = internals.requireImpl;

import { op_stream_base_register_state } from "ext:core/ops";
import { streamBaseState } from "ext:deno_node/internal_binding/stream_wrap.ts";
import "node:module";

let initialized = false;

function initialize(args) {
  const {
    usesLocalNodeModulesDir,
    argv0,
    runningOnMainThread,
    workerId,
    maybeWorkerMetadata,
    nodeDebug,
    warmup = false,
    moduleSpecifier = null,
  } = args;
  if (!warmup) {
    if (initialized) {
      throw new Error("Node runtime already initialized");
    }
    initialized = true;
    if (usesLocalNodeModulesDir) {
      requireImpl.setUsesLocalNodeModulesDir();
    }

    // FIXME(bartlomieju): not nice to depend on `Deno` namespace here
    // but it's the only way to get `args` and `version` and this point.
    internals.__bootstrapNodeProcess(
      argv0,
      Deno.args,
      Deno.version,
      nodeDebug ?? "",
      false,
      runningOnMainThread,
    );
    internals.__initWorkerThreads(
      runningOnMainThread,
      workerId,
      maybeWorkerMetadata,
      moduleSpecifier,
    );
    internals.__setupChildProcessIpcChannel();
    op_stream_base_register_state(streamBaseState);
    // `Deno[Deno.internal].requireImpl` will be unreachable after this line.
    delete internals.requireImpl;
  } else {
    // Warm up the process module
    internals.__bootstrapNodeProcess(
      undefined,
      undefined,
      undefined,
      undefined,
      true,
    );
  }
}

function loadCjsModule(moduleName, isMain, inspectBrk) {
  if (inspectBrk) {
    requireImpl.setInspectBrk();
  }
  requireImpl.Module._load(moduleName, null, { main: isMain });
}

globalThis.nodeBootstrap = initialize;

function closeIdleConnections() {
  // Close all idle connections in Node.js HTTP Agent pools.
  // This is called by the test runner before sanitizer checks to prevent
  // false positive resource leak detection for pooled keepAlive connections.
  // Destroy agents to close all pooled sockets.
  try {
    const http = nativeModuleExports["http"];
    if (http?.globalAgent) {
      http.globalAgent.destroy();
    }
  } catch {
    // Ignore
  }
  try {
    const https = nativeModuleExports["https"];
    if (https?.globalAgent) {
      https.globalAgent.destroy();
    }
  } catch {
    // Ignore
  }
}

internals.node = {
  initialize,
  loadCjsModule,
  closeIdleConnections,
};

const nativeModuleExports = requireImpl.nativeModuleExports;
nativeModuleExports["internal/console/constructor"].bindStreamsLazy(
  nativeModuleExports["console"],
  nativeModuleExports["process"],
);
