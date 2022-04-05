// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { pathFromURL } = window.__bootstrap.util;
  const { illegalConstructorKey } = window.__bootstrap.webUtil;
  const { ArrayPrototypeMap, ObjectEntries, String, TypeError } =
    window.__bootstrap.primordials;
  const { readableStreamForRid, writableStreamForRid } =
    window.__bootstrap.streamUtils;

  function spawn(command, {
    args = [],
    cwd = undefined,
    clearEnv = false,
    env = {},
    uid = undefined,
    gid = undefined,
    stdin = "null",
    stdout = "inherit",
    stderr = "inherit",
  } = {}) {
    const child = core.opSync("op_command_spawn", {
      cmd: pathFromURL(command),
      args: ArrayPrototypeMap(args, String),
      cwd: pathFromURL(cwd),
      clearEnv,
      env: ObjectEntries(env),
      uid,
      gid,
      stdin,
      stdout,
      stderr,
    });
    return new Child(illegalConstructorKey, child);
  }

  class Child {
    #rid;

    #pid;
    get pid() {
      return this.#pid;
    }

    #stdinRid;
    #stdin = null;
    get stdin() {
      return this.#stdin;
    }

    #stdoutRid;
    #stdout = null;
    get stdout() {
      return this.#stdout;
    }

    #stderrRid;
    #stderr = null;
    get stderr() {
      return this.#stderr;
    }

    constructor(key = null, {
      rid,
      pid,
      stdinRid,
      stdoutRid,
      stderrRid,
    } = null) {
      if (key !== illegalConstructorKey) {
        throw new TypeError("Illegal constructor.");
      }

      this.#rid = rid;
      this.#pid = pid;

      if (stdinRid !== null) {
        this.#stdinRid = stdinRid;
        this.#stdin = writableStreamForRid(stdinRid);
      }

      if (stdoutRid !== null) {
        this.#stdoutRid = stdoutRid;
        this.#stdout = readableStreamForRid(stdoutRid);
      }

      if (stderrRid !== null) {
        this.#stderrRid = stderrRid;
        this.#stderr = readableStreamForRid(stderrRid);
      }
    }

    #status;
    get status() {
      if (this.#status) {
        return this.#status;
      }
      const status = core.opSync("op_command_status", this.#rid);
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      if (status) {
        status.signal ??= undefined;
      }
      return status;
    }

    async wait() {
      if (this.#rid === null) {
        throw new TypeError("Child process has already terminated.");
      }
      const status = await core.opAsync(
        "op_command_wait",
        this.#rid,
        this.#stdinRid,
      );
      await this.stdin?.abort();
      this.#rid = null;
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      status.signal ??= undefined;
      this.#status = status;
      return status;
    }

    async output() {
      if (this.#rid === null) {
        throw new TypeError("Child process has already terminated.");
      }
      const res = await core.opAsync("op_command_output", {
        rid: this.#rid,
        stdoutRid: this.#stdoutRid,
        stderrRid: this.#stderrRid,
      });
      await this.stdin?.abort();
      this.#rid = null;
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      res.status.signal ??= undefined;
      this.#status = res.status;
      return res;
    }

    kill(signo) {
      if (this.#rid === null) {
        throw new TypeError("Child process has already terminated.");
      }
      core.opSync("op_kill", this.#pid, signo);
    }
  }

  function command(command, {
    stdin = "null",
    stdout = "piped",
    stderr = "piped",
    ...options
  } = {}) { // TODO: more options (like input)?
    const child = spawn(command, {
      stdin,
      stdout,
      stderr,
      ...options,
    });
    return child.output();
  }

  window.__bootstrap.command = {
    spawn,
    Child,
    command,
  };
})(this);
