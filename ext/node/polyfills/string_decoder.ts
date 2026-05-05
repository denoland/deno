// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/_string_decoder.ts");
export const { StringDecoder } = _mod;
export default _mod.default;
