// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/querystring.js");

export const {
  decode,
  encode,
  escape,
  parse,
  stringify,
  unescape,
  unescapeBuffer,
} = mod;

export default mod.default;
