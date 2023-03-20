// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const internals = globalThis.__bootstrap.internals;
const requireImpl = internals.requireImpl;
const primordials = globalThis.__bootstrap.primordials;
const { ObjectDefineProperty } = primordials;
import { nodeGlobals, nodeGlobalThis } from "ext:deno_node/00_globals.js";
import "ext:deno_node/01_require.js";

let initialized = false;

function initialize(
  nodeGlobalThisName,
  usesLocalNodeModulesDir,
  argv0,
) {
  if (initialized) {
    throw Error("Node runtime already initialized");
  }
  initialized = true;
  if (usesLocalNodeModulesDir) {
    requireImpl.setUsesLocalNodeModulesDir();
  }
  const nativeModuleExports = requireImpl.nativeModuleExports;
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
  // `Deno[Deno.internal].requireImpl` will be unreachable after this line.
  delete internals.requireImpl;
}

function loadCjsModule(moduleName, isMain, inspectBrk) {
  if (inspectBrk) {
    requireImpl.setInspectBrk();
  }
  requireImpl.Module._load(moduleName, null, { main: isMain });
}

internals.node = {
  initialize,
  loadCjsModule,
};
