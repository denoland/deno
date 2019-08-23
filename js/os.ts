// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { core } from "./core";
import * as dispatch from "./dispatch";
import { sendSync, msg, flatbuffers } from "./dispatch_flatbuffers";
import * as dispatchJson from "./dispatch_json";
import { assert } from "./util";
import * as util from "./util";
import { window } from "./window";

/** The current process id of the runtime. */
export let pid: number;

/** Reflects the NO_COLOR environment variable: https://no-color.org/ */
export let noColor: boolean;

function setGlobals(pid_: number, noColor_: boolean): void {
  assert(!pid);
  pid = pid_;
  noColor = noColor_;
}

/** Check if running in terminal.
 *
 *       console.log(Deno.isTTY().stdout);
 */
export function isTTY(): { stdin: boolean; stdout: boolean; stderr: boolean } {
  return dispatchJson.sendSync(dispatch.OP_IS_TTY);
}

/** Exit the Deno process with optional exit code. */
export function exit(code = 0): never {
  dispatchJson.sendSync(dispatch.OP_EXIT, { code });
  return util.unreachable();
}

function setEnv(key: string, value: string): void {
  const builder = flatbuffers.createBuilder();
  const key_ = builder.createString(key);
  const value_ = builder.createString(value);
  const inner = msg.SetEnv.createSetEnv(builder, key_, value_);
  sendSync(builder, msg.Any.SetEnv, inner);
}

/** Returns a snapshot of the environment variables at invocation. Mutating a
 * property in the object will set that variable in the environment for
 * the process. The environment object will only accept `string`s
 * as values.
 *
 *       const myEnv = Deno.env();
 *       console.log(myEnv.SHELL);
 *       myEnv.TEST_VAR = "HELLO";
 *       const newEnv = Deno.env();
 *       console.log(myEnv.TEST_VAR == newEnv.TEST_VAR);
 */
export function env(): { [index: string]: string } {
  const env = dispatchJson.sendSync(dispatch.OP_ENV);
  return new Proxy(env, {
    set(obj, prop: string, value: string): boolean {
      setEnv(prop, value);
      return Reflect.set(obj, prop, value);
    }
  });
}

/** Send to the privileged side that we have setup and are ready. */
function sendStart(): msg.StartRes {
  const builder = flatbuffers.createBuilder();
  const startOffset = msg.Start.createStart(builder, 0 /* unused */);
  const baseRes = sendSync(builder, msg.Any.Start, startOffset);
  assert(baseRes != null);
  assert(msg.Any.StartRes === baseRes!.innerType());
  const startResMsg = new msg.StartRes();
  assert(baseRes!.inner(startResMsg) != null);
  return startResMsg;
}

// This function bootstraps an environment within Deno, it is shared both by
// the runtime and the compiler environments.
// @internal
export function start(
  preserveDenoNamespace = true,
  source?: string
): msg.StartRes {
  core.setAsyncHandler(dispatch.asyncMsgFromRust);

  // First we send an empty `Start` message to let the privileged side know we
  // are ready. The response should be a `StartRes` message containing the CLI
  // args and other info.
  const startResMsg = sendStart();

  util.setLogDebug(startResMsg.debugFlag(), source);

  setGlobals(startResMsg.pid(), startResMsg.noColor());

  if (preserveDenoNamespace) {
    util.immutableDefine(window, "Deno", window.Deno);
    // Deno.core could ONLY be safely frozen here (not in globals.ts)
    // since shared_queue.js will modify core properties.
    Object.freeze(window.Deno.core);
    // core.sharedQueue is an object so we should also freeze it.
    Object.freeze(window.Deno.core.sharedQueue);
  } else {
    // Remove window.Deno
    delete window.Deno;
    assert(window.Deno === undefined);
  }

  return startResMsg;
}

/**
 * Returns the current user's home directory.
 * Requires the `--allow-env` flag.
 */
export function homeDir(): string {
  const builder = flatbuffers.createBuilder();
  const inner = msg.HomeDir.createHomeDir(builder);
  const baseRes = sendSync(builder, msg.Any.HomeDir, inner)!;
  assert(msg.Any.HomeDirRes === baseRes.innerType());
  const res = new msg.HomeDirRes();
  assert(baseRes.inner(res) != null);
  const path = res.path();

  if (!path) {
    throw new Error("Could not get home directory.");
  }

  return path;
}

/**
 * Returns the path to the current deno executable.
 * Requires the `--allow-env` flag.
 */
export function execPath(): string {
  return dispatchJson.sendSync(dispatch.OP_EXEC_PATH);
}
