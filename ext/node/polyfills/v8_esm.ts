// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/v8.ts");

export const {
  cachedDataVersionTag,
  getHeapCodeStatistics,
  getHeapSnapshot,
  getHeapSpaceStatistics,
  getHeapStatistics,
  queryObjects,
  setFlagsFromString,
  stopCoverage,
  takeCoverage,
  writeHeapSnapshot,
  serialize,
  deserialize,
  GCProfiler,
  Serializer,
  Deserializer,
  DefaultSerializer,
  DefaultDeserializer,
} = mod;

export default mod;
