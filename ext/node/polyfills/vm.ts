// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { notImplemented } from "ext:deno_node/_utils.ts";

const { core } = globalThis.__bootstrap;
const ops = core.ops;

export class Script {
  code: string;
  constructor(code: string, _options = {}) {
    this.code = `${code}`;
  }

  runInThisContext(_options: any) {
    const [result, error] = core.evalContext(this.code, "data:");
    if (error) {
      throw error.thrown;
    }
    return result;
  }

  runInContext(_contextifiedObject: any, _options: any) {
    notImplemented("Script.prototype.runInContext");
  }

  runInNewContext(contextObject: any, options: any) {
    if (options) {
      console.warn(
        "Script.runInNewContext options are currently not supported",
      );
    }
    return ops.op_vm_run_in_new_context(this.code, contextObject);
  }

  createCachedData() {
    notImplemented("Script.prototyp.createCachedData");
  }
}

export function createContext(_contextObject: any, _options: any) {
  notImplemented("createContext");
}

export function createScript(code: string, options: any) {
  return new Script(code, options);
}

export function runInContext(
  _code: string,
  _contextifiedObject: any,
  _options: any,
) {
  notImplemented("runInContext");
}

export function runInNewContext(
  code: string,
  contextObject: any,
  options: any,
) {
  if (options) {
    console.warn("vm.runInNewContext options are currently not supported");
  }
  return ops.op_vm_run_in_new_context(code, contextObject);
}

export function runInThisContext(
  code: string,
  options: any,
) {
  return createScript(code, options).runInThisContext(options);
}

export function isContext(_maybeContext: any) {
  // TODO(@littledivy): Currently we do not expose contexts so this is always false.
  return false;
}

export function compileFunction(_code: string, _params: any, _options: any) {
  notImplemented("compileFunction");
}

export function measureMemory(_options: any) {
  notImplemented("measureMemory");
}

export default {
  Script,
  createContext,
  createScript,
  runInContext,
  runInNewContext,
  runInThisContext,
  isContext,
  compileFunction,
  measureMemory,
};
