// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file

import { internals } from "ext:core/mod.js";
const requireImpl = internals.requireImpl;

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

internals.node = {
  initialize,
  loadCjsModule,
};

const nativeModuleExports = requireImpl.nativeModuleExports;
globalThis.Buffer = nativeModuleExports["buffer"].Buffer;
globalThis.clearImmediate = nativeModuleExports["timers"].clearImmediate;
globalThis.clearInterval = nativeModuleExports["timers"].clearInterval;
globalThis.clearTimeout = nativeModuleExports["timers"].clearTimeout;
globalThis.global = globalThis;
globalThis.process = nativeModuleExports["process"];
globalThis.setImmediate = nativeModuleExports["timers"].setImmediate;
globalThis.setInterval = nativeModuleExports["timers"].setInterval;
globalThis.setTimeout = nativeModuleExports["timers"].setTimeout;

nativeModuleExports["internal/console/constructor"].bindStreamsLazy(
  nativeModuleExports["console"],
  nativeModuleExports["process"],
);
