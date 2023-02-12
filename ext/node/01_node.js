// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const internals = globalThis.__bootstrap.internals;
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypePush,
  ArrayPrototypeFilter,
  ObjectEntries,
  ObjectCreate,
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

const nativeModuleExports = ObjectCreate(null);
const builtinModules = [];

function initialize(nodeModules, nodeGlobalThisName) {
  assert(!initialized);
  initialized = true;
  for (const [name, exports] of ObjectEntries(nodeModules)) {
    nativeModuleExports[name] = exports;
    ArrayPrototypePush(builtinModules, name);
  }
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
}

internals.node = {
  globalThis: nodeGlobalThis,
  initialize,
  nativeModuleExports,
  builtinModules,
};
