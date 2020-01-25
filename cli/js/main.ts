// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is entry point for "main" isolate, ie. the one
// that is create when you run Deno executable.
//
// It provides global scope as `window`.

import {
  readOnly,
  writable,
  nonEnumerable,
  windowOrWorkerGlobalScopeMethods,
  windowOrWorkerGlobalScopeProperties,
  eventTargetProperties
} from "./globals.ts";
import * as domTypes from "./dom_types.ts";
import { assert, log } from "./util.ts";
import * as os from "./os.ts";
import { args } from "./deno.ts";
import * as csprng from "./get_random_values.ts";
import { setPrepareStackTrace } from "./error_stack.ts";
import { replLoop } from "./repl.ts";
import { setVersions } from "./version.ts";
import { setLocation } from "./location.ts";
import { setBuildInfo } from "./build.ts";
import { setSignals } from "./process.ts";
import * as Deno from "./deno.ts";
import { internalObject } from "./internals.ts";

// TODO: half of this stuff should be called in worker as well...
function bootstrapMainRuntime(): void {
  const s = os.start(true);

  setBuildInfo(s.os, s.arch);
  setSignals();
  setVersions(s.denoVersion, s.v8Version, s.tsVersion);

  setPrepareStackTrace(Error);

  if (s.mainModule) {
    assert(s.mainModule.length > 0);
    setLocation(s.mainModule);
  }
  log("cwd", s.cwd);
  for (let i = 0; i < s.argv.length; i++) {
    args.push(s.argv[i]);
  }
  log("args", args);
  Object.freeze(args);

  if (!s.mainModule) {
    replLoop();
  }
}

// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// @ts-ignore
Deno[Deno.symbols.internal] = internalObject;

export const mainRuntimeGlobalProperties = {
  bootstrapMainRuntime: nonEnumerable(bootstrapMainRuntime),
  window: readOnly(globalThis),
  Deno: readOnly(Deno),

  crypto: readOnly(csprng),
  // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
  // it seems those two properties should be availble to workers as well
  onload: writable(undefined),
  onunload: writable(undefined)
};

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
