// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

import { notImplemented } from "ext:deno_node/_utils.ts";

const { core } = globalThis.__bootstrap;

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

  runInNewContext(_contextObject: any, _options: any) {
    notImplemented("Script.prototype.runInNewContext");
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
  _code: string,
  _contextObject: any,
  _options: any,
) {
  notImplemented("runInNewContext");
}

export function runInThisContext(
  code: string,
  options: any,
) {
  return createScript(code, options).runInThisContext(options);
}

export function isContext(_maybeContext: any) {
  notImplemented("isContext");
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
