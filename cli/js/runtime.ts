// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "./core.ts";
import * as Deno from "./deno.ts";
import * as dispatchMinimal from "./ops/dispatch_minimal.ts";
import * as dispatchJson from "./ops/dispatch_json.ts";
import { assert } from "./util.ts";
import * as util from "./util.ts";
import { setBuildInfo } from "./build.ts";
import { LocationImpl } from "./web/location.ts";
import { setPrepareStackTrace } from "./error_stack.ts";
import { Start, start as startOp } from "./ops/runtime.ts";
import { setSignals } from "./process.ts";
import { symbols } from "./symbols.ts";
import { internalObject } from "./internals.ts";

interface Version {
  deno: string;
  v8: string;
  typescript: string;
}

export let OPS_CACHE: { [name: string]: number };

export function getAsyncHandler(opName: string): (msg: Uint8Array) => void {
  switch (opName) {
    case "op_write":
    case "op_read":
      return dispatchMinimal.asyncMsgFromRust;
    default:
      return dispatchJson.asyncMsgFromRust;
  }
}

// TODO(bartlomieju): temporary solution, must be fixed when moving
// dispatches to separate crates
export function initOps(): void {
  OPS_CACHE = core.ops();
  for (const [name, opId] of Object.entries(OPS_CACHE)) {
    core.setAsyncHandler(opId, getAsyncHandler(name));
  }
}

/**
 * This function bootstraps JS runtime, unfortunately some of runtime
 * code depends on information like "os" and thus getting this information
 * is required at startup.
 */
export function start(preserveDenoNamespace = true, source?: string): Start {
  initOps();
  // First we send an empty `Start` message to let the privileged side know we
  // are ready. The response should be a `StartRes` message containing the CLI
  // args and other info.
  const s = startOp();

  // Add internal object to Deno object.
  // This is not exposed as part of the Deno types.
  // @ts-ignore
  Deno[symbols.internal] = internalObject;
  // Build info is used by internal code, so setting it first.
  setBuildInfo(s.os, s.arch);

  util.setLogDebug(s.debugFlag, source);
  util.immutableDefine(globalThis, "location", new LocationImpl(s.location));
  Object.freeze(globalThis.location);
  setPrepareStackTrace(Error);
  setSignals();

  if (preserveDenoNamespace) {
    const version: Version = {
      deno: s.denoVersion,
      v8: s.v8Version,
      typescript: s.tsVersion
    };
    Object.freeze(version);
    util.immutableDefine(Deno, "version", version);
    util.immutableDefine(Deno, "pid", s.pid);
    util.immutableDefine(Deno, "noColor", s.noColor);
    util.immutableDefine(Deno, "args", [...s.args]);
    // TODO(bartlomieju): Object.freeze should be called recursively on
    // all properties of `globalThis.Deno`
    Object.freeze(Deno);
    util.immutableDefine(globalThis, "Deno", Deno);
    // Deno.core could ONLY be safely frozen here (not in globals.ts)
    // since shared_queue.js will modify core properties.
    Object.freeze(globalThis.Deno.core);
    // core.sharedQueue is an object so we should also freeze it.
    Object.freeze(globalThis.Deno.core.sharedQueue);
  } else {
    // Remove globalThis.Deno
    delete globalThis.Deno;
    assert(globalThis.Deno === undefined);
  }

  util.log("cwd", s.cwd);
  util.log("args", Deno.args);
  return s;
}
