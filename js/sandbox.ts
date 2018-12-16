// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { libdeno } from "./libdeno";
import { globalEval } from "./global_eval";

const window = globalEval("this");

export interface DenoSandbox {
  // tslint:disable-next-line:no-any
  env: any;
  // tslint:disable-next-line:no-any
  eval: (code: string) => any;
}

function formatFrameAtMessage(frame: { [key: string]: string }) {
  if (frame.functionName) {
    return `    at ${frame.functionName} (${frame.scriptName}:${frame.line}:${
      frame.column
    })`;
  } else if (frame.isEval) {
    return `    at eval (${frame.scriptName}:${frame.line}:${frame.column})`;
  } else {
    return `    at ${frame.scriptName}:${frame.line}:${frame.column}`;
  }
}

class DenoSandboxImpl implements DenoSandbox {
  constructor(public env: {}) {}
  eval(code: string) {
    const [result, errMsg] = libdeno.runInContext(this.env, code);
    if (errMsg) {
      let err;
      try {
        const errInfo = JSON.parse(errMsg);
        err = new Error();
        err.message = errInfo.message; // Don't prefix with "Error"
        err.stack = `${errInfo.message}\n${errInfo.frames
          .map((frame: { [key: string]: string }) =>
            formatFrameAtMessage(frame)
          )
          .join("\n")}`;
      } catch (e) {
        err = new Error("Unknown sandbox error");
      }
      throw err;
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
  // Copy necessary window properties first
  // To avoid `window.Error !== Error` that causes unexpected behavior
  for (const key of Object.getOwnPropertyNames(window)) {
    try {
      env[key] = model[key];
    } catch (e) {}
  }
  // Then the actual model
  for (const key in model) {
    env[key] = model[key];
  }

  return new DenoSandboxImpl(env);
}
