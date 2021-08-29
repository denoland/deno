// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { pathFromURL } = window.__bootstrap.util;
  const { read } = window.__bootstrap.io;
  const { writeAll } = window.__bootstrap.buffer;
  const { illegalConstructorKey } = window.__bootstrap.webUtil;
  const { ArrayPrototypeMap, ObjectEntries, String, TypeError, Uint8Array } =
    window.__bootstrap.primordials;

  class Command {
    #options;

    constructor(command, {
      args = [],
      cwd = undefined,
      clearEnv = false,
      env = {},
    } = {}) {
      this.#options = {
        cmd: pathFromURL(command),
        args: ArrayPrototypeMap(args, String),
        cwd: pathFromURL(cwd),
        clearEnv,
        env: ObjectEntries(env),
      };
    }

    spawn(options = {}) {
      const child = core.opSync("op_command_spawn", {
        ...this.#options,
        ...options,
      });

      return new Child(illegalConstructorKey, child);
    }

    async status(options = {}) {
      return await core.opAsync("op_command_status", {
        ...this.#options,
        ...options,
      });
    }

    async output() {
      return await core.opAsync("op_command_output", this.#options);
    }
  }

  class Child {
    #rid;

    #pid;
    get pid() {
      return this.#pid;
    }

    #stdin;
    get stdin() {
      return this.#stdin;
    }

    #stdoutRid;
    #stdout;
    get stdout() {
      return this.#stdout;
    }

    #stderrRid;
    #stderr;
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

      if (stdinRid !== undefined) {
        this.#stdin = new WritableStream({
          async write(chunk) {
            await writeAll(stdinRid, chunk);
          },
          abort() {
            core.close(stdinRid);
          },
        });
      }

      if (stdoutRid !== undefined) {
        this.#stdoutRid = stdoutRid;
        // TODO(crowlkats): BYOB Stream
        this.#stdout = new ReadableStream({
          async pull(controller) {
            const buf = new Uint8Array(16384);
            const res = await read(stdoutRid, buf);
            if (res === null) {
              core.close(stdoutRid);
              controller.close();
            } else {
              controller.enqueue(buf);
            }
          },
          cancel() {
            core.close(stdoutRid);
          },
        });
      }

      if (stderrRid !== undefined) {
        this.#stderrRid = stderrRid;
        // TODO(crowlkats): BYOB Stream
        this.#stderr = new ReadableStream({
          async pull(controller) {
            const buf = new Uint8Array(16384);
            const res = await read(stderrRid, buf);
            if (res === null) {
              core.close(stderrRid);
              controller.close();
            } else {
              controller.enqueue(buf);
            }
          },
          cancel() {
            core.close(stderrRid);
          },
        });
      }
    }

    get status() {
      return core.opSync("op_command_child_status", this.#rid);
    }

    async wait() {
      const res = await core.opAsync("op_command_child_wait", this.#rid);
      await this.stdin?.close();
      return res;
    }

    async output() {
      const res = await core.opAsync("op_command_child_output", {
        rid: this.#rid,
        stdoutRid: this.#stdoutRid,
        stderrRid: this.#stderrRid,
      });
      await this.stdin?.close();
      return res;
    }

    kill(signal) {
      core.opSync("op_kill", this.#pid, signal);
    }
  }

  window.__bootstrap.command = {
    Command,
    Child,
  };
})(this);
