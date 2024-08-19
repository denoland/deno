// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  op_delete_env,
  op_env,
  op_exec_path,
  op_exit,
  op_get_env,
  op_get_exit_code,
  op_gid,
  op_hostname,
  op_loadavg,
  op_network_interfaces,
  op_os_release,
  op_os_uptime,
  op_set_env,
  op_set_exit_code,
  op_system_memory_info,
  op_uid,
} from "ext:core/ops";
const {
  Error,
  FunctionPrototypeBind,
  NumberIsInteger,
  RangeError,
  SymbolFor,
  TypeError,
} = primordials;

import { Event, EventTarget } from "ext:deno_web/02_event.js";

const windowDispatchEvent = FunctionPrototypeBind(
  EventTarget.prototype.dispatchEvent,
  globalThis,
);

function loadavg() {
  return op_loadavg();
}

function hostname() {
  return op_hostname();
}

function osRelease() {
  return op_os_release();
}

function osUptime() {
  return op_os_uptime();
}

function systemMemoryInfo() {
  return op_system_memory_info();
}

function networkInterfaces() {
  return op_network_interfaces();
}

function gid() {
  return op_gid();
}

function uid() {
  return op_uid();
}

// This is an internal only method used by the test harness to override the
// behavior of exit when the exit sanitizer is enabled.
let exitHandler = null;
function setExitHandler(fn) {
  exitHandler = fn;
}

function exit(code) {
  // Set exit code first so unload event listeners can override it.
  if (typeof code === "number") {
    op_set_exit_code(code);
  } else {
    code = op_get_exit_code();
  }

  // Dispatches `unload` only when it's not dispatched yet.
  if (!globalThis[SymbolFor("Deno.isUnloadDispatched")]) {
    // Invokes the `unload` hooks before exiting
    // ref: https://github.com/denoland/deno/issues/3603
    windowDispatchEvent(new Event("unload"));
  }

  if (exitHandler) {
    exitHandler(code);
    return;
  }

  op_exit();
  throw new Error("Code not reachable");
}

function getExitCode() {
  return op_get_exit_code();
}

function setExitCode(value) {
  if (typeof value !== "number") {
    throw new TypeError(
      `Exit code must be a number, got: ${value} (${typeof value})`,
    );
  }
  if (!NumberIsInteger(value)) {
    throw new RangeError(
      `Exit code must be an integer, got: ${value}`,
    );
  }
  op_set_exit_code(value);
}

function setEnv(key, value) {
  op_set_env(key, value);
}

function getEnv(key) {
  return op_get_env(key) ?? undefined;
}

function deleteEnv(key) {
  op_delete_env(key);
}

const env = {
  get: getEnv,
  toObject() {
    return op_env();
  },
  set: setEnv,
  has(key) {
    return getEnv(key) !== undefined;
  },
  delete: deleteEnv,
};

function execPath() {
  return op_exec_path();
}

export {
  env,
  execPath,
  exit,
  getExitCode,
  gid,
  hostname,
  loadavg,
  networkInterfaces,
  osRelease,
  osUptime,
  setExitCode,
  setExitHandler,
  systemMemoryInfo,
  uid,
};
