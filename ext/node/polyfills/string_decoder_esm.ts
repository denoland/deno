// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/string_decoder.ts");

export const { StringDecoder } = mod;

export default mod.default;
