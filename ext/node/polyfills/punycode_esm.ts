// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/punycode.ts");

export const { decode, encode, toASCII, toUnicode, ucs2, version } = mod;

export default mod.default;
