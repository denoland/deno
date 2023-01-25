// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const { ops } = Deno[Deno.internal].core;

export function notImplemented(msg) {
  const message = msg ? `Not implemented: ${msg}` : "Not implemented";
  throw new Error(message);
}

export function cachedDataVersionTag() {
  return ops.op_v8_cached_data_version_tag();
}

export function getHeapCodeStatistics() {
  notImplemented("v8.getHeapCodeStatistics");
}

export function getHeapSnapshot() {
  notImplemented("v8.getHeapSnapshot");
}

export function getHeapSpaceStatistics() {
  notImplemented("v8.getHeapSpaceStatistics");
}

export function getHeapStatistics() {
  notImplemented("v8.getHeapStatistics");
}

export function setFlagsFromString(args) {
  ops.op_v8_set_flags_from_string(args);
}

export function stopCoverage() {
  notImplemented("v8.stopCoverage");
}

export function takeCoverage() {
  notImplemented("v8.takeCoverage");
}

export function writeHeapSnapshot() {
  notImplemented("v8.writeHeapSnapshot");
}

export function serialize() {
  notImplemented("v8.serialize");
}

export function deserialize() {
  notImplemented("v8.deserialize");
}

export class Serializer {
  constructor() {
    notImplemented("v8.Serializer.prototype.constructor");
  }
}

export class Deserializer {
  constructor() {
    notImplemented("v8.Deserializer.prototype.constructor");
  }
}

export class DefaultSerializer {
  constructor() {
    notImplemented("v8.DefaultSerializer.prototype.constructor");
  }
}

export class DefaultDeserializer {
  constructor() {
    notImplemented("v8.DefaultDeserializer.prototype.constructor");
  }
}

export const promiseHooks = {
  onInit() {
    notImplemented("v8.promiseHooks.onInit");
  },
  onSettled() {
    notImplemented("v8.promiseHooks.onSetttled");
  },
  onBefore() {
    notImplemented("v8.promiseHooks.onBefore");
  },
  createHook() {
    notImplemented("v8.promiseHooks.createHook");
  },
};

export default {
  cachedDataVersionTag,
  getHeapCodeStatistics,
  getHeapSnapshot,
  getHeapSpaceStatistics,
  getHeapStatistics,
  setFlagsFromString,
  stopCoverage,
  takeCoverage,
  writeHeapSnapshot,
  serialize,
  deserialize,
  Serializer,
  Deserializer,
  DefaultSerializer,
  DefaultDeserializer,
  promiseHooks,
};
