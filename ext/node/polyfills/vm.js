// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/_vm.js");
export const {
  Module,
  Script,
  SourceTextModule,
  constants,
  createContext,
  createScript,
  runInContext,
  runInNewContext,
  runInThisContext,
  isContext,
  compileFunction,
  measureMemory,
} = _mod;
export default _mod.default;
