// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const internals = globalThis.__bootstrap.internals;
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypePush,
  ArrayPrototypeFilter,
  ObjectDefineProperty,
  Proxy,
  ReflectDefineProperty,
  ReflectDeleteProperty,
  ReflectGet,
  ReflectGetOwnPropertyDescriptor,
  ReflectHas,
  ReflectOwnKeys,
  ReflectSet,
  Set,
  SetPrototypeHas,
} = primordials;

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

let initialized = false;
const nodeGlobals = {};
const nodeGlobalThis = new Proxy(globalThis, {
  get(target, prop) {
    if (ReflectHas(nodeGlobals, prop)) {
      return ReflectGet(nodeGlobals, prop);
    } else {
      return ReflectGet(target, prop);
    }
  },
  set(target, prop, value) {
    if (ReflectHas(nodeGlobals, prop)) {
      return ReflectSet(nodeGlobals, prop, value);
    } else {
      return ReflectSet(target, prop, value);
    }
  },
  has(target, prop) {
    return ReflectHas(nodeGlobals, prop) || ReflectHas(target, prop);
  },
  deleteProperty(target, prop) {
    const nodeDeleted = ReflectDeleteProperty(nodeGlobals, prop);
    const targetDeleted = ReflectDeleteProperty(target, prop);
    return nodeDeleted || targetDeleted;
  },
  ownKeys(target) {
    const targetKeys = ReflectOwnKeys(target);
    const nodeGlobalsKeys = ReflectOwnKeys(nodeGlobals);
    const nodeGlobalsKeySet = new Set(nodeGlobalsKeys);
    return [
      ...ArrayPrototypeFilter(
        targetKeys,
        (k) => !SetPrototypeHas(nodeGlobalsKeySet, k),
      ),
      ...nodeGlobalsKeys,
    ];
  },
  defineProperty(target, prop, desc) {
    if (ReflectHas(nodeGlobals, prop)) {
      return ReflectDefineProperty(nodeGlobals, prop, desc);
    } else {
      return ReflectDefineProperty(target, prop, desc);
    }
  },
  getOwnPropertyDescriptor(target, prop) {
    if (ReflectHas(nodeGlobals, prop)) {
      return ReflectGetOwnPropertyDescriptor(nodeGlobals, prop);
    } else {
      return ReflectGetOwnPropertyDescriptor(target, prop);
    }
  },
});

function initialize(nodeModules, nodeGlobalThisName, argv0) {
  assert(!initialized);
  initialized = true;
  internals.require.setupBuiltinModules(nodeModules);
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

internals.node = {
  globalThis: nodeGlobalThis,
  initialize,
};
