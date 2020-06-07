// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "main" isolate, ie. the one
// that is created when you run "deno" executable.
//
// It provides a single function that should be called by Rust:
//  - `bootstrapMainRuntime` - must be called once, when Isolate is created.
//   It sets up runtime by providing globals for `WindowScope` and adds `Deno` global.

import * as denoNs from "./deno.ts";
import * as denoUnstableNs from "./deno_unstable.ts";
import { exit } from "./ops/os.ts";
import {
  readOnly,
  getterOnly,
  writable,
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties,
  setEventTargetData,
} from "./globals.ts";
import { unstableMethods, unstableProperties } from "./globals_unstable.ts";
import { internalObject, internalSymbol } from "./internals.ts";
import { setSignals } from "./signals.ts";
import { replLoop } from "./repl.ts";
import { setTimeout } from "./web/timers.ts";
import * as runtime from "./runtime.ts";
import { log, immutableDefine } from "./util.ts";

// TODO: factor out `Deno` global assignment to separate function
// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(denoNs as any)[internalSymbol] = internalObject;

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
  // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
  // it seems those two properties should be available to workers as well
  onload: writable(null),
  onunload: writable(null),
  close: writable(windowClose),
  closed: getterOnly(() => windowIsClosing),
};

let hasBootstrapped = false;

export function bootstrapMainRuntime(): void {
  if (hasBootstrapped) {
    throw new Error("Worker runtime already bootstrapped");
  }
  // Remove bootstrapping methods from global scope
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).bootstrap = undefined;
  log("bootstrapMainRuntime");
  hasBootstrapped = true;
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
  Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
  Object.defineProperties(globalThis, eventTargetProperties);
  Object.defineProperties(globalThis, mainRuntimeGlobalProperties);
  setEventTargetData(globalThis);
  // Registers the handler for window.onload function.
  globalThis.addEventListener("load", (e) => {
    const { onload } = globalThis;
    if (typeof onload === "function") {
      onload(e);
    }
  });
  // Registers the handler for window.onunload function.
  globalThis.addEventListener("unload", (e) => {
    const { onunload } = globalThis;
    if (typeof onunload === "function") {
      onunload(e);
    }
  });

  const { args, cwd, noColor, pid, repl, unstableFlag } = runtime.start();

  Object.defineProperties(denoNs, {
    pid: readOnly(pid),
    noColor: readOnly(noColor),
    args: readOnly(Object.freeze(args)),
  });

  if (unstableFlag) {
    Object.defineProperties(globalThis, unstableMethods);
    Object.defineProperties(globalThis, unstableProperties);
    Object.assign(denoNs, denoUnstableNs);
  }

  // Setup `Deno` global - we're actually overriding already
  // existing global `Deno` with `Deno` namespace from "./deno.ts".
  immutableDefine(globalThis, "Deno", denoNs);
  Object.freeze(globalThis.Deno);
  Object.freeze(globalThis.Deno.core);
  Object.freeze(globalThis.Deno.core.sharedQueue);
  setSignals();

  log("cwd", cwd);
  log("args", args);

  if (repl) {
    replLoop();
  }
}
