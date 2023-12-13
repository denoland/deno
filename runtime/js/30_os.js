// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
import { Event, EventTarget } from "ext:deno_web/02_event.js";
const {
  Error,
  FunctionPrototypeBind,
  SymbolFor,
} = primordials;

const windowDispatchEvent = FunctionPrototypeBind(
  EventTarget.prototype.dispatchEvent,
  globalThis,
);

function loadavg() {
  return ops.op_loadavg();
}

function hostname() {
  return ops.op_hostname();
}

function osRelease() {
  return ops.op_os_release();
}

function osUptime() {
  return ops.op_os_uptime();
}

function systemMemoryInfo() {
  return ops.op_system_memory_info();
}

function networkInterfaces() {
  return ops.op_network_interfaces();
}

function gid() {
  return ops.op_gid();
}

function uid() {
  return ops.op_uid();
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
    ops.op_set_exit_code(code);
  } else {
    code = 0;
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

  ops.op_exit();
  throw new Error("Code not reachable");
}

function setEnv(key, value) {
  ops.op_set_env(key, value);
}

function getEnv(key) {
  return ops.op_get_env(key) ?? undefined;
}

function deleteEnv(key) {
  ops.op_delete_env(key);
}

const env = {
  get: getEnv,
  toObject() {
    return ops.op_env();
  },
  set: setEnv,
  has(key) {
    return getEnv(key) !== undefined;
  },
  delete: deleteEnv,
};

function execPath() {
  return ops.op_exec_path();
}

export {
  env,
  execPath,
  exit,
  gid,
  hostname,
  loadavg,
  networkInterfaces,
  osRelease,
  osUptime,
  setExitHandler,
  systemMemoryInfo,
  uid,
};
