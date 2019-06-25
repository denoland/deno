// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import { core } from "./core";
import { handleAsyncMsgFromRust, sendSync } from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as util from "./util";
import { window } from "./window";

/** The current process id of the runtime. */
export let pid: number;

/** Reflects the NO_COLOR environment variable: https://no-color.org/ */
export let noColor: boolean;

/** Path to the current deno process's executable file. */
export let execPath: string;

function setGlobals(pid_: number, noColor_: boolean, execPath_: string): void {
  assert(!pid);
  pid = pid_;
  noColor = noColor_;
  execPath = execPath_;
}

/** Check if running in terminal.
 *
 *       console.log(Deno.isTTY().stdout);
 */
export function isTTY(): { stdin: boolean; stdout: boolean; stderr: boolean } {
  const builder = flatbuffers.createBuilder();
  const inner = msg.IsTTY.createIsTTY(builder);
  const baseRes = sendSync(builder, msg.Any.IsTTY, inner)!;
  assert(msg.Any.IsTTYRes === baseRes.innerType());
  const res = new msg.IsTTYRes();
  assert(baseRes.inner(res) != null);

  return { stdin: res.stdin(), stdout: res.stdout(), stderr: res.stderr() };
}

/** Exit the Deno process with optional exit code. */
export function exit(exitCode = 0): never {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Exit.createExit(builder, exitCode);
  sendSync(builder, msg.Any.Exit, inner);
  return util.unreachable();
}

function setEnv(key: string, value: string): void {
  const builder = flatbuffers.createBuilder();
  const key_ = builder.createString(key);
  const value_ = builder.createString(value);
  const inner = msg.SetEnv.createSetEnv(builder, key_, value_);
  sendSync(builder, msg.Any.SetEnv, inner);
}

function createEnv(inner: msg.EnvironRes): { [index: string]: string } {
  const env: { [index: string]: string } = {};

  for (let i = 0; i < inner.mapLength(); i++) {
    const item = inner.map(i)!;
    env[item.key()!] = item.value()!;
  }

  return new Proxy(env, {
    set(obj, prop: string, value: string): boolean {
      setEnv(prop, value);
      return Reflect.set(obj, prop, value);
    }
  });
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
  /* Ideally we could write
  const res = sendSync({
    command: msg.Command.ENV,
  });
  */
  const builder = flatbuffers.createBuilder();
  const inner = msg.Environ.createEnviron(builder);
  const baseRes = sendSync(builder, msg.Any.Environ, inner)!;
  assert(msg.Any.EnvironRes === baseRes.innerType());
  const res = new msg.EnvironRes();
  assert(baseRes.inner(res) != null);
  // TypeScript cannot track assertion above, therefore not null assertion
  return createEnv(res);
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
export function start(source?: string): msg.StartRes {
  core.setAsyncHandler(handleAsyncMsgFromRust);

  // First we send an empty `Start` message to let the privileged side know we
  // are ready. The response should be a `StartRes` message containing the CLI
  // args and other info.
  const startResMsg = sendStart();

  util.setLogDebug(startResMsg.debugFlag(), source);

  setGlobals(startResMsg.pid(), startResMsg.noColor(), startResMsg.execPath()!);

  // Deno.core could ONLY be safely frozen here (not in globals.ts)
  // since shared_queue.js will modify core properties.
  Object.freeze(window.Deno.core);

  return startResMsg;
}

/**
 * Returns the current user's home directory.
 * Does not require elevated privileges.
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
