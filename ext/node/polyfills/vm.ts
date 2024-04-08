// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  op_vm_create_context,
  op_vm_create_script,
  op_vm_is_context,
  op_vm_script_run_in_context,
  op_vm_script_run_in_this_context,
} from "ext:core/ops";

export class Script {
  #inner;

  constructor(code: string, _options = {}) {
    this.#inner = op_vm_create_script(code);
  }

  runInThisContext(_options: any) {
    return op_vm_script_run_in_this_context(this.#inner);
  }

  runInContext(contextifiedObject: any, _options: any) {
    return op_vm_script_run_in_context(this.#inner, contextifiedObject);
  }

  runInNewContext(contextObject: any, options: any) {
    const context = createContext(contextObject);
    return this.runInContext(context, options);
  }

  createCachedData() {
    notImplemented("Script.prototype.createCachedData");
  }
}

export function createContext(contextObject: any = {}, _options: any) {
  if (isContext(contextObject)) {
    return contextObject;
  }

  op_vm_create_context(contextObject);
  return contextObject;
}

export function createScript(code: string, options: any) {
  return new Script(code, options);
}

export function runInContext(
  code: string,
  contextifiedObject: any,
  _options: any,
) {
  return createScript(code).runInContext(contextifiedObject);
}

export function runInNewContext(
  code: string,
  contextObject: any,
  options: any,
) {
  if (options) {
    console.warn("vm.runInNewContext options are currently not supported");
  }
  return createScript(code).runInNewContext(contextObject);
}

export function runInThisContext(
  code: string,
  options: any,
) {
  return createScript(code, options).runInThisContext(options);
}

export function isContext(maybeContext: any) {
  return op_vm_is_context(maybeContext);
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
