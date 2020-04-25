// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { core } from "./core.ts";
import * as dispatchMinimal from "./ops/dispatch_minimal.ts";
import * as dispatchJson from "./ops/dispatch_json.ts";
import * as util from "./util.ts";
import { setBuildInfo } from "./build.ts";
import { setVersions } from "./version.ts";
import { setPrepareStackTrace } from "./error_stack.ts";
import { Start, start as startOp } from "./ops/runtime.ts";
import { handleTimerMacrotask } from "./web/timers.ts";

export let OPS_CACHE: { [name: string]: number };

function getAsyncHandler(opName: string): (msg: Uint8Array) => void {
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
  core.setMacrotaskCallback(handleTimerMacrotask);
}

export function start(source?: string): Start {
  initOps();
  // First we send an empty `Start` message to let the privileged side know we
  // are ready. The response should be a `StartRes` message containing the CLI
  // args and other info.
  const s = startOp();

  setVersions(s.denoVersion, s.v8Version, s.tsVersion);
  setBuildInfo(s.os, s.arch);
  util.setLogDebug(s.debugFlag, source);

  setPrepareStackTrace(Error);
  return s;
}
