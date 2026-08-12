// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/stream/consumers.js");

export const {
  arrayBuffer,
  blob,
  buffer,
  bytes,
  text,
  json,
} = mod;

export default mod;
