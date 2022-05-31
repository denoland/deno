// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const {
    Error,
    SymbolFor,
  } = window.__bootstrap.primordials;

  const windowDispatchEvent = window.dispatchEvent.bind(window);

  function loadavg() {
    return core.opSync("op_loadavg");
  }

  function hostname() {
    return core.opSync("op_hostname");
  }

  function osRelease() {
    return core.opSync("op_os_release");
  }

  function systemMemoryInfo() {
    return core.opSync("op_system_memory_info");
  }

  function networkInterfaces() {
    return core.opSync("op_network_interfaces");
  }

  function getGid() {
    return core.opSync("op_getgid");
  }

  function getUid() {
    return core.opSync("op_getuid");
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
      core.opSync("op_set_exit_code", code);
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

    core.opSync("op_exit");
    throw new Error("Code not reachable");
  }

  function setEnv(key, value) {
    core.opSync("op_set_env", key, value);
  }

  function getEnv(key) {
    return core.opSync("op_get_env", key) ?? undefined;
  }

  function deleteEnv(key) {
    core.opSync("op_delete_env", key);
  }

  const env = {
    get: getEnv,
    toObject() {
      return core.opSync("op_env");
    },
    set: setEnv,
    delete: deleteEnv,
  };

  function execPath() {
    return core.opSync("op_exec_path");
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
