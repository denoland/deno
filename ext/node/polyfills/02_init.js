// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

import { core, internals } from "ext:core/mod.js";
const requireImpl = internals.requireImpl;

import { nodeGlobals } from "ext:deno_node/00_globals.js";
import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";
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
    );
    internals.__initWorkerThreads(
      runningOnMainThread,
      workerId,
      maybeWorkerMetadata,
      moduleSpecifier,
    );
    internals.__setupChildProcessIpcChannel();
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
  const { internalRidSymbol } = core;

  // Collect resource IDs that need to be closed.
  // We do this in two phases:
  // 1. Destroy sockets via agent.destroy() (graceful cleanup)
  // 2. Close any remaining resources directly (force cleanup)
  const ridsToClose = [];

  function collectRidsFromAgent(agent) {
    if (!agent) return;
    const sets = [agent.freeSockets, agent.sockets];
    for (const set of sets) {
      if (!set) continue;
      for (const key of Object.keys(set)) {
        const sockets = set[key];
        if (!Array.isArray(sockets)) continue;
        for (const socket of sockets) {
          try {
            const stream = socket?._handle?.[kStreamBaseField];
            if (stream) {
              const rid = stream[internalRidSymbol];
              if (rid !== undefined) {
                ridsToClose.push(rid);
              }
            }
          } catch {
            // Ignore
          }
        }
      }
    }
  }

  // Phase 1: Collect RIDs and destroy agents
  try {
    const http = nativeModuleExports["http"];
    if (http?.globalAgent) {
      collectRidsFromAgent(http.globalAgent);
      http.globalAgent.destroy();
    }
  } catch {
    // Ignore
  }
  try {
    const https = nativeModuleExports["https"];
    if (https?.globalAgent) {
      collectRidsFromAgent(https.globalAgent);
      https.globalAgent.destroy();
    }
  } catch {
    // Ignore
  }

  // Phase 2: Force close any remaining resources
  // This handles the case where socket.destroy() is a no-op due to HandleWrap state
  for (const rid of ridsToClose) {
    try {
      core.tryClose(rid);
    } catch {
      // Ignore - already closed
    }
  }
}

internals.node = {
  initialize,
  loadCjsModule,
  closeIdleConnections,
};

const nativeModuleExports = requireImpl.nativeModuleExports;
nodeGlobals.Buffer = nativeModuleExports["buffer"].Buffer;
nodeGlobals.clearImmediate = nativeModuleExports["timers"].clearImmediate;
nodeGlobals.clearInterval = nativeModuleExports["timers"].clearInterval;
nodeGlobals.clearTimeout = nativeModuleExports["timers"].clearTimeout;
nodeGlobals.global = globalThis;
nodeGlobals.process = nativeModuleExports["process"];
nodeGlobals.setImmediate = nativeModuleExports["timers"].setImmediate;
nodeGlobals.setInterval = nativeModuleExports["timers"].setInterval;
nodeGlobals.setTimeout = nativeModuleExports["timers"].setTimeout;

nativeModuleExports["internal/console/constructor"].bindStreamsLazy(
  nativeModuleExports["console"],
  nativeModuleExports["process"],
);
