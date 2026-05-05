// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/_querystring.js");
export const {
  decode,
  encode,
  escape,
  parse,
  stringify,
  unescape,
  unescapeBuffer,
} = _mod;
export default _mod.default;
