// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function loadavg() {
    return core.jsonOpSync("op_loadavg");
  }

  function hostname() {
    return core.jsonOpSync("op_hostname");
  }

  function osRelease() {
    return core.jsonOpSync("op_os_release");
  }

  function systemMemoryInfo() {
    return core.jsonOpSync("op_system_memory_info");
  }

  function systemCpuInfo() {
    const { cores, speed } = core.jsonOpSync("op_system_cpu_info");
    // Map nulls to undefined for compatibility
    return {
      cores: cores ?? undefined,
      speed: speed ?? undefined,
    };
  }

  // This is an internal only method used by the test harness to override the
  // behavior of exit when the exit sanitizer is enabled.
  let exitHandler = null;
  function setExitHandler(fn) {
    exitHandler = fn;
  }

  function exit(code = 0) {
    // Dispatches `unload` only when it's not dispatched yet.
    if (!window[Symbol.for("isUnloadDispatched")]) {
      // Invokes the `unload` hooks before exiting
      // ref: https://github.com/denoland/deno/issues/3603
      window.dispatchEvent(new Event("unload"));
    }

    if (exitHandler) {
      exitHandler(code);
      return;
    }

    core.jsonOpSync("op_exit", code);
    throw new Error("Code not reachable");
  }

  function setEnv(key, value) {
    core.jsonOpSync("op_set_env", { key, value });
  }

  function getEnv(key) {
    return core.jsonOpSync("op_get_env", key) ?? undefined;
  }

  function deleteEnv(key) {
    core.jsonOpSync("op_delete_env", key);
  }

  const env = {
    get: getEnv,
    toObject() {
      return core.jsonOpSync("op_env");
    },
    set: setEnv,
    delete: deleteEnv,
  };

  function execPath() {
    // modified for ci testing
    console.log(2)
    return core.jsonOpSync("op_exec_path");
  }

  window.__bootstrap.os = {
    env,
    execPath,
    setExitHandler,
    exit,
    osRelease,
    systemMemoryInfo,
    systemCpuInfo,
    hostname,
    loadavg,
  };
})(this);
