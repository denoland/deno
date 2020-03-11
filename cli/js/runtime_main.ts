// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "main" isolate, ie. the one
// that is created when you run "deno" executable.
//
// It provides a single function that should be called by Rust:
//  - `bootstrapMainRuntime` - must be called once, when Isolate is created.
//   It sets up runtime by providing globals for `WindowScope` and adds `Deno` global.

import * as Deno from "./deno.ts";
import * as domTypes from "./web/dom_types.ts";
import * as csprng from "./ops/get_random_values.ts";
import {
  readOnly,
  writable,
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties
} from "./globals.ts";
import { internalObject } from "./internals.ts";
import { setSignals } from "./signals.ts";
import { replLoop } from "./repl.ts";
import * as runtime from "./runtime.ts";
import { symbols } from "./symbols.ts";
import { log } from "./util.ts";

// TODO: factor out `Deno` global assignment to separate function
// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// @ts-ignore
Deno[symbols.internal] = internalObject;

export const mainRuntimeGlobalProperties = {
  window: readOnly(globalThis),
  self: readOnly(globalThis),
  Deno: readOnly(Deno),

  crypto: readOnly(csprng),
  // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
  // it seems those two properties should be availble to workers as well
  onload: writable(undefined),
  onunload: writable(undefined)
};

let hasBootstrapped = false;

export function bootstrapMainRuntime(): void {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }
  log("bootstrapMainRuntime");
  hasBootstrapped = true;
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
  Object.defineProperties(globalThis, eventTargetProperties);
  Object.defineProperties(globalThis, mainRuntimeGlobalProperties);
  // Registers the handler for window.onload function.
  globalThis.addEventListener("load", (e: domTypes.Event): void => {
    const { onload } = globalThis;
    if (typeof onload === "function") {
      onload(e);
    }
  });
  // Registers the handler for window.onunload function.
  globalThis.addEventListener("unload", (e: domTypes.Event): void => {
    const { onunload } = globalThis;
    if (typeof onunload === "function") {
      onunload(e);
    }
  });

  const s = runtime.start(true);
  setSignals();

  log("cwd", s.cwd);
  for (let i = 0; i < s.args.length; i++) {
    Deno.args.push(s.args[i]);
  }
  log("args", Deno.args);
  Object.freeze(Deno.args);

  if (s.repl) {
    replLoop();
  }
}
