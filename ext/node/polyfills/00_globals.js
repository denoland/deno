// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypeFilter,
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

export { nodeGlobals, nodeGlobalThis };
