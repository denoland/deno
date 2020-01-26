// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module is the entry point for "main" isolate, ie. the one
// that is created when you run "deno" executable.
//
// It provides a single that should be called by Rust:
//  - `bootstrapWorkerRuntime` - must be called once, when Isolate is created.
//   It sets up runtime by providing globals for `WindowScope` and adds `Deno` global.

import {
  readOnly,
  writable,
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

// TODO: factor out `Deno` global assignment to separate function
// Add internal object to Deno object.
// This is not exposed as part of the Deno types.
// @ts-ignore
Deno[Deno.symbols.internal] = internalObject;

export const mainRuntimeGlobalProperties = {
  window: readOnly(globalThis),
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

  // TODO: half of this stuff should be called in worker as well...
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
