// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
const primordials = globalThis.__bootstrap.primordials;
import { pathFromURL } from "internal:runtime/js/06_util.js";
import { add, remove } from "internal:deno_web/03_abort_signal.js";
const {
  ArrayPrototypeMap,
  ObjectEntries,
  ObjectPrototypeIsPrototypeOf,
  String,
  TypeError,
  PromisePrototypeThen,
  SafePromiseAll,
  SymbolFor,
  Symbol,
} = primordials;
import {
  readableStreamCollectIntoUint8Array,
  readableStreamForRidUnrefable,
  readableStreamForRidUnrefableRef,
  readableStreamForRidUnrefableUnref,
  ReadableStreamPrototype,
  writableStreamForRid,
} from "internal:deno_web/06_streams.js";

const illegalConstructorKey = Symbol("illegalConstructorKey");

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
  return new ChildProcess(illegalConstructorKey, {
    ...child,
    signal,
  });
}

function spawnChild(command, options = {}) {
  return spawnChildInner(
    ops.op_spawn_child,
    command,
    "Deno.Command().spawn()",
    options,
  );
}

function collectOutput(readableStream) {
  if (
    !(ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, readableStream))
  ) {
    return null;
  }

  return readableStreamCollectIntoUint8Array(readableStream);
}

class ChildProcess {
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

  #stdoutRid;
  #stdout = null;
  get stdout() {
    if (this.#stdout == null) {
      throw new TypeError("stdout is not piped");
    }
    return this.#stdout;
  }

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

    const { 0: status, 1: stdout, 2: stderr } = await SafePromiseAll([
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

function spawn(command, options) {
  if (options?.stdin === "piped") {
    throw new TypeError(
      "Piped stdin is not supported for this function, use 'Deno.Command().spawn()' instead",
    );
  }
  return spawnChildInner(
    ops.op_spawn_child,
    command,
    "Deno.Command().output()",
    options,
  )
    .output();
}

function spawnSync(command, {
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
      "Piped stdin is not supported for this function, use 'Deno.Command().spawn()' instead",
    );
  }
  const result = ops.op_spawn_sync({
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
}

class Command {
  #command;
  #options;

  constructor(command, options) {
    this.#command = command;
    this.#options = options;
  }

  output() {
    if (this.#options?.stdin === "piped") {
      throw new TypeError(
        "Piped stdin is not supported for this function, use 'Deno.Command.spawn()' instead",
      );
    }
    return spawn(this.#command, this.#options);
  }

  outputSync() {
    if (this.#options?.stdin === "piped") {
      throw new TypeError(
        "Piped stdin is not supported for this function, use 'Deno.Command.spawn()' instead",
      );
    }
    return spawnSync(this.#command, this.#options);
  }

  spawn() {
    const options = {
      ...(this.#options ?? {}),
      stdout: this.#options?.stdout ?? "inherit",
      stderr: this.#options?.stderr ?? "inherit",
      stdin: this.#options?.stdin ?? "inherit",
    };
    return spawnChild(this.#command, options);
  }
}

export { ChildProcess, Command };
