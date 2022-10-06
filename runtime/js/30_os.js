// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const {
    Error,
    SymbolFor,
  } = window.__bootstrap.primordials;

  const windowDispatchEvent = window.dispatchEvent.bind(window);

  function loadavg() {
    return ops.op_loadavg();
  }

  function hostname() {
    return ops.op_hostname();
  }

  function osRelease() {
    return ops.op_os_release();
  }

  function systemMemoryInfo() {
    return ops.op_system_memory_info();
  }

  function networkInterfaces() {
    return ops.op_network_interfaces();
  }

  function getGid() {
    return ops.op_getgid();
  }

  function getUid() {
    return ops.op_getuid();
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
    if (!window[SymbolFor("isUnloadDispatched")]) {
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
    delete: deleteEnv,
  };

  function execPath() {
    return ops.op_exec_path();
  }

  window.__bootstrap.os = {
    env,
    execPath,
    exit,
    getGid,
    getUid,
    hostname,
    loadavg,
    networkInterfaces,
    osRelease,
    setExitHandler,
    systemMemoryInfo,
  };
})(this);
