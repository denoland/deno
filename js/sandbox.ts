// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { libdeno } from "./libdeno";

export interface DenoSandbox {
  // tslint:disable-next-line:no-any
  env: any;
  // tslint:disable-next-line:no-any
  eval: (code: string) => any;
}

class DenoSandboxImpl implements DenoSandbox {
  constructor(public env: {}) {}
  eval(code: string) {
    const [result, errMsg] = libdeno.runInContext(this.env, code);
    if (errMsg) {
      throw new Error(errMsg);
    }
    return result;
  }
}

/** Create a sandboxed context (with a model) to execute code inside.
 *
 *       import * as deno from "deno";
 *       const s = deno.sandbox({a: 1});
 *       s.b = 2;
 *       s.eval("const c = a + b");
 *       console.log(s.c) // prints "3"
 */
export function sandbox(
  model: any // tslint:disable-line:no-any
): DenoSandbox {
  if (typeof model !== "object") {
    throw new Error("Sandbox model has to be an object!");
  }
  // env is the global object of context
  const env = libdeno.makeContext();
  for (const key in model) {
    env[key] = model[key];
  }

  return new DenoSandboxImpl(env);
}
