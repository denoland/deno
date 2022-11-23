// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const { pathFromURL } = window.__bootstrap.util;
  const { illegalConstructorKey } = window.__bootstrap.webUtil;
  const { add, remove } = window.__bootstrap.abortSignal;
  const {
    ArrayPrototypeMap,
    ObjectEntries,
    String,
    TypeError,
    PromisePrototypeThen,
    SafePromiseAll,
    SymbolFor,
  } = window.__bootstrap.primordials;
  const {
    readableStreamCollectIntoUint8Array,
    readableStreamForRidUnrefable,
    readableStreamForRidUnrefableRef,
    readableStreamForRidUnrefableUnref,
    writableStreamForRid,
  } = window.__bootstrap.streams;

  const promiseIdSymbol = SymbolFor("Deno.core.internalPromiseId");

  function spawnChildInner(opFn, command, apiName, {
    args = [],
    cwd = undefined,
    clearEnv = false,
    env = {},
    uid = undefined,
    gid = undefined,
    stdin = "null",
    stdout = "piped",
    stderr = "piped",
    signal = undefined,
    windowsRawArguments = false,
  } = {}) {
    const child = opFn({
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
      windowsRawArguments,
    }, apiName);
    return new Child(illegalConstructorKey, {
      ...child,
      signal,
    });
  }

  function createSpawnChild(opFn) {
    return function spawnChild(command, options = {}) {
      return spawnChildInner(opFn, command, "Deno.spawnChild()", options);
    };
  }

  function collectOutput(readableStream) {
    if (!(readableStream instanceof ReadableStream)) {
      return null;
    }

    return readableStreamCollectIntoUint8Array(readableStream);
  }

  class Child {
    #rid;
    #waitPromiseId;
    #unrefed = false;

    #pid;
    get pid() {
      return this.#pid;
    }

    #stdin = null;
    get stdin() {
      if (this.#stdin == null) {
        throw new TypeError("stdin is not piped");
      }
      return this.#stdin;
    }

    #stdoutPromiseId;
    #stdoutRid;
    #stdout = null;
    get stdout() {
      if (this.#stdout == null) {
        throw new TypeError("stdout is not piped");
      }
      return this.#stdout;
    }

    #stderrPromiseId;
    #stderrRid;
    #stderr = null;
    get stderr() {
      if (this.#stderr == null) {
        throw new TypeError("stderr is not piped");
      }
      return this.#stderr;
    }

    constructor(key = null, {
      signal,
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
        this.#stdin = writableStreamForRid(stdinRid);
      }

      if (stdoutRid !== null) {
        this.#stdoutRid = stdoutRid;
        this.#stdout = readableStreamForRidUnrefable(stdoutRid);
      }

      if (stderrRid !== null) {
        this.#stderrRid = stderrRid;
        this.#stderr = readableStreamForRidUnrefable(stderrRid);
      }

      const onAbort = () => this.kill("SIGTERM");
      signal?.[add](onAbort);

      const waitPromise = core.opAsync("op_spawn_wait", this.#rid);
      this.#waitPromiseId = waitPromise[promiseIdSymbol];
      this.#status = PromisePrototypeThen(waitPromise, (res) => {
        this.#rid = null;
        signal?.[remove](onAbort);
        return res;
      });
    }

    #status;
    get status() {
      return this.#status;
    }

    async output() {
      if (this.#stdout?.locked) {
        throw new TypeError(
          "Can't collect output because stdout is locked",
        );
      }
      if (this.#stderr?.locked) {
        throw new TypeError(
          "Can't collect output because stderr is locked",
        );
      }

      const [status, stdout, stderr] = await SafePromiseAll([
        this.#status,
        collectOutput(this.#stdout),
        collectOutput(this.#stderr),
      ]);

      return {
        success: status.success,
        code: status.code,
        signal: status.signal,
        get stdout() {
          if (stdout == null) {
            throw new TypeError("stdout is not piped");
          }
          return stdout;
        },
        get stderr() {
          if (stderr == null) {
            throw new TypeError("stderr is not piped");
          }
          return stderr;
        },
      };
    }

    kill(signo = "SIGTERM") {
      if (this.#rid === null) {
        throw new TypeError("Child process has already terminated.");
      }
      ops.op_kill(this.#pid, signo, "Deno.Child.kill()");
    }

    ref() {
      this.#unrefed = false;
      core.refOp(this.#waitPromiseId);
      if (this.#stdout) readableStreamForRidUnrefableRef(this.#stdout);
      if (this.#stderr) readableStreamForRidUnrefableRef(this.#stderr);
    }

    unref() {
      this.#unrefed = true;
      core.unrefOp(this.#waitPromiseId);
      if (this.#stdout) readableStreamForRidUnrefableUnref(this.#stdout);
      if (this.#stderr) readableStreamForRidUnrefableUnref(this.#stderr);
    }
  }

  function createSpawn(opFn) {
    return function spawn(command, options) {
      if (options?.stdin === "piped") {
        throw new TypeError(
          "Piped stdin is not supported for this function, use 'Deno.spawnChild()' instead",
        );
      }
      return spawnChildInner(opFn, command, "Deno.spawn()", options).output();
    };
  }

  function createSpawnSync(opFn) {
    return function spawnSync(command, {
      args = [],
      cwd = undefined,
      clearEnv = false,
      env = {},
      uid = undefined,
      gid = undefined,
      stdin = "null",
      stdout = "piped",
      stderr = "piped",
      windowsRawArguments = false,
    } = {}) {
      if (stdin === "piped") {
        throw new TypeError(
          "Piped stdin is not supported for this function, use 'Deno.spawnChild()' instead",
        );
      }
      const result = opFn({
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
        windowsRawArguments,
      });
      return {
        success: result.status.success,
        code: result.status.code,
        signal: result.status.signal,
        get stdout() {
          if (result.stdout == null) {
            throw new TypeError("stdout is not piped");
          }
          return result.stdout;
        },
        get stderr() {
          if (result.stderr == null) {
            throw new TypeError("stderr is not piped");
          }
          return result.stderr;
        },
      };
    };
  }

  class Command {
    #command;
    #options;

    #child;

    #consumed;

    constructor(command, options) {
      this.#command = command;
      this.#options = options;
    }

    output() {
      if (this.#child) {
        return this.#child.output();
      } else {
        if (this.#consumed) {
          throw new TypeError(
            "Command instance is being or has already been consumed.",
          );
        }
        if (this.#options?.stdin === "piped") {
          throw new TypeError(
            "Piped stdin is not supported for this function, use 'Deno.Command.spawn()' instead",
          );
        }

        this.#consumed = true;
        return Deno.spawn(this.#command, this.#options);
      }
    }

    outputSync() {
      if (this.#consumed) {
        throw new TypeError(
          "Command instance is being or has already been consumed.",
        );
      }
      if (this.#child) {
        throw new TypeError("Was spawned");
      }
      if (this.#options?.stdin === "piped") {
        throw new TypeError(
          "Piped stdin is not supported for this function, use 'Deno.Command.spawn()' instead",
        );
      }

      this.#consumed = true;
      return Deno.spawnSync(this.#command, this.#options);
    }

    spawn() {
      if (this.#consumed) {
        throw new TypeError(
          "Command instance is being or has already been consumed.",
        );
      }

      this.#consumed = true;
      this.#child = Deno.spawnChild(this.#command, this.#options);
    }

    get stdin() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      return this.#child.stdin;
    }

    get stdout() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      return this.#child.stdout;
    }

    get stderr() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      return this.#child.stderr;
    }

    get status() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      return this.#child.status;
    }

    get pid() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      return this.#child.pid;
    }

    kill(signo = "SIGTERM") {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }
      this.#child.kill(signo);
    }

    ref() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      this.#child.ref();
    }

    unref() {
      if (!this.#child) {
        throw new TypeError("Wasn't spawned");
      }

      this.#child.unref();
    }
  }

  window.__bootstrap.spawn = {
    Child,
    Command,
    createSpawn,
    createSpawnChild,
    createSpawnSync,
  };
})(this);
