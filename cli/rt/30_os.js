// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const sendSync = window.__bootstrap.dispatchJson.sendSync;

  function loadavg() {
    return sendSync("op_loadavg");
  }

  function hostname() {
    return sendSync("op_hostname");
  }

  function osRelease() {
    return sendSync("op_os_release");
  }

  function exit(code = 0) {
    sendSync("op_exit", { code });
    throw new Error("Code not reachable");
  }

  function setEnv(key, value) {
    sendSync("op_set_env", { key, value });
  }

  function getEnv(key) {
    return sendSync("op_get_env", { key })[0];
  }

  function deleteEnv(key) {
    sendSync("op_delete_env", { key });
  }

  const env = {
    get: getEnv,
    toObject() {
      return sendSync("op_env");
    },
    set: setEnv,
    delete: deleteEnv,
  };

  function execPath() {
    return sendSync("op_exec_path");
  }

  window.__bootstrap.os = {
    env,
    execPath,
    exit,
    osRelease,
    hostname,
    loadavg,
  };
})(this);
