// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { notImplemented } from "ext:deno_node/_utils.ts";

const { ops } = globalThis.__bootstrap.core;

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

const buffer = new Float64Array(14);

export function getHeapStatistics() {
  ops.op_v8_get_heap_statistics(buffer);

  return {
    total_heap_size: buffer[0],
    total_heap_size_executable: buffer[1],
    total_physical_size: buffer[2],
    total_available_size: buffer[3],
    used_heap_size: buffer[4],
    heap_size_limit: buffer[5],
    malloced_memory: buffer[6],
    peak_malloced_memory: buffer[7],
    does_zap_garbage: buffer[8],
    number_of_native_contexts: buffer[9],
    number_of_detached_contexts: buffer[10],
    total_global_handles_size: buffer[11],
    used_global_handles_size: buffer[12],
    external_memory: buffer[13],
  };
}

export function setFlagsFromString() {
  // NOTE(bartlomieju): From Node.js docs:
  // The v8.setFlagsFromString() method can be used to programmatically set V8
  // command-line flags. This method should be used with care. Changing settings
  // after the VM has started may result in unpredictable behavior, including
  // crashes and data loss; or it may simply do nothing.
  //
  // Notice: "or it may simply do nothing". This is what we're gonna do,
  // this function will just be a no-op.
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
