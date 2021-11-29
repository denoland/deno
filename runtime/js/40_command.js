// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { pathFromURL } = window.__bootstrap.util;
  const { read, write } = window.__bootstrap.io;
  const { illegalConstructorKey } = window.__bootstrap.webUtil;
  const { ArrayPrototypeMap, ObjectEntries, String, TypeError } =
    window.__bootstrap.primordials;

  function createWritableIOStream(rid) {
    return new WritableStream({
      async write(chunk) {
        let nwritten = 0;
        while (nwritten < chunk.byteLength) {
          nwritten += await write(rid, chunk.subarray(nwritten));
        }
      },
      abort() {
        core.tryClose(rid);
      },
    });
  }

  function createReadableIOStream(rid) {
    return new ReadableStream({
      async pull(controller) {
        const view = controller.byobRequest.view;
        const res = await read(rid, view);
        if (res === null) {
          core.close(rid);
          controller.close();
        } else {
          controller.byobRequest.respond(res);
        }
      },
      cancel() {
        core.close(rid);
      },
      type: "bytes",
      autoAllocateChunkSize: 16384,
    });
  }

  class Command {
    #rid;

    constructor(command, {
      args = [],
      cwd = undefined,
      clearEnv = false,
      env = {},
      uid = undefined,
      gid = undefined,
    } = {}) {
      this.#rid = core.opSync("op_create_command", {
        cmd: pathFromURL(command),
        args: ArrayPrototypeMap(args, String),
        cwd: pathFromURL(cwd),
        clearEnv,
        env: ObjectEntries(env),
        uid,
        gid,
      });
    }

    spawn(options = {}) {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      const child = core.opSync("op_command_spawn", this.#rid, options);
      this.#rid = null;
      return new Child(illegalConstructorKey, child);
    }

    async status(options = {}) {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      const status = await core.opAsync(
        "op_command_status",
        this.#rid,
        options,
      );
      this.#rid = null;
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      status.signal ??= undefined;
      return status;
    }

    async output() {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      const res = await core.opAsync("op_command_output", this.#rid);
      this.#rid = null;
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      res.status.signal ??= undefined;
      return res;
    }
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
        this.#stdin = createWritableIOStream(stdinRid);
      }

      if (stdoutRid !== null) {
        this.#stdoutRid = stdoutRid;
        this.#stdout = createReadableIOStream(stdoutRid);
      }

      if (stderrRid !== null) {
        this.#stderrRid = stderrRid;
        this.#stderr = createReadableIOStream(stderrRid);
      }
    }

    get status() {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      const status = core.opSync("op_command_child_status", this.#rid);
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      status.signal ??= undefined;
      return status;
    }

    async wait() {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      const status = await core.opAsync(
        "op_command_child_wait",
        this.#rid,
        this.#stdinRid,
      );
      await this.stdin?.abort();
      this.#rid = null;
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      status.signal ??= undefined;
      return status;
    }

    async output() {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      const res = await core.opAsync("op_command_child_output", {
        rid: this.#rid,
        stdoutRid: this.#stdoutRid,
        stderrRid: this.#stderrRid,
      });
      await this.stdin?.abort();
      this.#rid = null;
      // TODO(@crowlKats): 2.0 change typings to return null instead of undefined for status.signal
      res.status.signal ??= undefined;
      return res;
    }

    kill(signo) {
      if (this.#rid === null) {
        throw new TypeError("Child has already been used up.");
      }
      core.opSync("op_kill", this.#pid, signo);
    }
  }

  window.__bootstrap.command = {
    Command,
    Child,
  };
})(this);
