// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/util.ts");

export const {
  _extend,
  aborted,
  callbackify,
  convertProcessSignalToExitCode,
  debuglog,
  deprecate,
  format,
  formatWithOptions,
  getCallSites,
  getSystemErrorMessage,
  getSystemErrorName,
  inherits,
  inspect,
  isArray,
  isDeepStrictEqual,
  log,
  MIMEParams,
  MIMEType,
  parseArgs,
  parseEnv,
  promisify,
  stripVTControlCharacters,
  styleText,
  TextDecoder,
  TextEncoder,
  toUSVString,
  types,
} = mod;

export const debug = debuglog;

export default mod;
