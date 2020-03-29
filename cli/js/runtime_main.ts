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
import { exit } from "./ops/os.ts";
import {
  readOnly,
  getterOnly,
  writable,
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties,
} from "./globals.ts";
import { internalObject } from "./internals.ts";
import { setSignals } from "./signals.ts";
import { replLoop } from "./repl.ts";
import { LocationImpl } from "./web/location.ts";
import { setTimeout } from "./web/timers.ts";
import * as runtime from "./runtime.ts";
import { symbols } from "./symbols.ts";
import { log, immutableDefine } from "./util.ts";

// TODO: factor out `Deno` global assignment to separate function
// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// @ts-ignore
Deno[symbols.internal] = internalObject;

let windowIsClosing = false;

function windowClose(): void {
  if (!windowIsClosing) {
    windowIsClosing = true;
    // Push a macrotask to exit after a promise resolve.
    // This is not perfect, but should be fine for first pass.
    Promise.resolve().then(() =>
      setTimeout.call(
        null,
        () => {
          // This should be fine, since only Window/MainWorker has .close()
          exit(0);
        },
        0
      )
    );
  }
}

export const mainRuntimeGlobalProperties = {
  window: readOnly(globalThis),
  self: readOnly(globalThis),
  crypto: readOnly(csprng),
  // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
  // it seems those two properties should be availble to workers as well
  onload: writable(undefined),
  onunload: writable(undefined),
  close: writable(windowClose),
  closed: getterOnly(() => windowIsClosing),
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

  const s = runtime.start();

  const location = new LocationImpl(s.location);
  immutableDefine(globalThis, "location", location);
  Object.freeze(globalThis.location);

  Object.defineProperties(Deno, {
    pid: readOnly(s.pid),
    noColor: readOnly(s.noColor),
    args: readOnly(Object.freeze(s.args)),
  });
  // Setup `Deno` global - we're actually overriding already
  // existing global `Deno` with `Deno` namespace from "./deno.ts".
  immutableDefine(globalThis, "Deno", Deno);
  Object.freeze(globalThis.Deno);
  Object.freeze(globalThis.Deno.core);
  Object.freeze(globalThis.Deno.core.sharedQueue);
  setSignals();

  log("cwd", s.cwd);
  log("args", Deno.args);

  if (s.repl) {
    replLoop();
  }
}
