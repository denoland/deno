// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/url.ts");

export const {
  URL,
  URLSearchParams,
  urlToHttpOptions,
  Url,
  parse,
  format,
  resolve,
  resolveObject,
  domainToASCII,
  domainToUnicode,
  fileURLToPath,
  pathToFileURL,
} = mod;

export default mod;
