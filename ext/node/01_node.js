// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const internals = globalThis.__bootstrap.internals;
const primordials = globalThis.__bootstrap.primordials;
const { ObjectDefineProperty } = primordials;
import {
  nodeGlobals,
  nodeGlobalThis,
} from "ext:deno_node_loading/00_globals.js";

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

let initialized = false;

function initialize(
  nodeModules,
  nodeGlobalThisName,
  usesLocalNodeModulesDir,
  argv0,
) {
  assert(!initialized);
  initialized = true;
  internals.require.setupBuiltinModules(nodeModules);
  if (usesLocalNodeModulesDir) {
    internals.require.setUsesLocalNodeModulesDir();
  }
  const nativeModuleExports = internals.require.nativeModuleExports;
  nodeGlobals.Buffer = nativeModuleExports["buffer"].Buffer;
  nodeGlobals.clearImmediate = nativeModuleExports["timers"].clearImmediate;
  nodeGlobals.clearInterval = nativeModuleExports["timers"].clearInterval;
  nodeGlobals.clearTimeout = nativeModuleExports["timers"].clearTimeout;
  nodeGlobals.console = nativeModuleExports["console"];
  nodeGlobals.global = nodeGlobalThis;
  nodeGlobals.process = nativeModuleExports["process"];
  nodeGlobals.setImmediate = nativeModuleExports["timers"].setImmediate;
  nodeGlobals.setInterval = nativeModuleExports["timers"].setInterval;
  nodeGlobals.setTimeout = nativeModuleExports["timers"].setTimeout;

  // add a hidden global for the esm code to use in order to reliably
  // get node's globalThis
  ObjectDefineProperty(globalThis, nodeGlobalThisName, {
    enumerable: false,
    writable: false,
    value: nodeGlobalThis,
  });
  // FIXME(bartlomieju): not nice to depend on `Deno` namespace here
  // but it's the only way to get `args` and `version` and this point.
  internals.__bootstrapNodeProcess(argv0, Deno.args, Deno.version);
}

function loadCjsModule(moduleName, isMain, inspectBrk) {
  if (inspectBrk) {
    internals.require.setInspectBrk();
  }
  internals.require.Module._load(moduleName, null, { main: isMain });
}

internals.node = {
  globalThis: nodeGlobalThis,
  initialize,
  loadCjsModule,
};
