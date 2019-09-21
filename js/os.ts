// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { core } from "./core.ts";
import { JsonOp } from "./dispatch_json.ts";
import { assert } from "./util.ts";
import * as util from "./util.ts";
import { window } from "./window.ts";
import { OperatingSystem, Arch } from "./build.ts";

// builtin modules
import { _setGlobals } from "./deno.ts";

const OP_IS_TTY = new JsonOp("is_tty");

/** Check if running in terminal.
 *
 *       console.log(Deno.isTTY().stdout);
 */
export function isTTY(): { stdin: boolean; stdout: boolean; stderr: boolean } {
  return OP_IS_TTY.sendSync();
}

const OP_EXIT = new JsonOp("exit");

/** Exit the Deno process with optional exit code. */
export function exit(code = 0): never {
  OP_EXIT.sendSync({ code });
  return util.unreachable();
}

const OP_SET_ENV = new JsonOp("set_env");

function setEnv(key: string, value: string): void {
  OP_SET_ENV.sendSync({ key, value });
}

const OP_ENV = new JsonOp("env");

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
  const env = OP_ENV.sendSync();
  return new Proxy(env, {
    set(obj, prop: string, value: string): boolean {
      setEnv(prop, value);
      return Reflect.set(obj, prop, value);
    }
  });
}

interface Start {
  cwd: string;
  pid: number;
  argv: string[];
  mainModule: string; // Absolute URL.
  debugFlag: boolean;
  depsFlag: boolean;
  typesFlag: boolean;
  versionFlag: boolean;
  denoVersion: string;
  v8Version: string;
  tsVersion: string;
  noColor: boolean;
  xevalDelim: string;
  os: OperatingSystem;
  arch: Arch;
}

const OP_START = new JsonOp("start");

// This function bootstraps an environment within Deno, it is shared both by
// the runtime and the compiler environments.
// @internal
export function start(preserveDenoNamespace = true, source?: string): Start {
  core.initOps();
  // @ts-ignore
  window.console.error("op start", OP_START, OP_START.opId);
  // First we send an empty `Start` message to let the privileged side know we
  // are ready. The response should be a `StartRes` message containing the CLI
  // args and other info.
  const s = OP_START.sendSync();

  util.setLogDebug(s.debugFlag, source);

  // pid and noColor need to be set in the Deno module before it's set to be
  // frozen.
  _setGlobals(s.pid, s.noColor);
  delete window.Deno._setGlobals;
  Object.freeze(window.Deno);

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

  return s;
}

const OP_HOME_DIR = new JsonOp("home_dir");

/**
 * Returns the current user's home directory.
 * Requires the `--allow-env` flag.
 */
export function homeDir(): string {
  const path = OP_HOME_DIR.sendSync();
  if (!path) {
    throw new Error("Could not get home directory.");
  }
  return path;
}

const OP_EXEC_PATH = new JsonOp("exec_path");
/**
 * Returns the path to the current deno executable.
 * Requires the `--allow-env` flag.
 */
export function execPath(): string {
  return OP_EXEC_PATH.sendSync();
}
