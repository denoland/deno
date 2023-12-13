// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const ops = core.ops;
const {
  ArrayPrototypeMap,
  ArrayPrototypeSlice,
  TypeError,
  ObjectEntries,
  SafeArrayIterator,
  String,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  SafePromiseAll,
  Symbol,
} = primordials;
import { FsFile } from "ext:deno_fs/30_fs.js";
import { readAll } from "ext:deno_io/12_io.js";
import {
  assert,
  pathFromURL,
  SymbolAsyncDispose,
} from "ext:deno_web/00_infra.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import {
  readableStreamCollectIntoUint8Array,
  readableStreamForRidUnrefable,
  readableStreamForRidUnrefableRef,
  readableStreamForRidUnrefableUnref,
  ReadableStreamPrototype,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";

function opKill(pid, signo, apiName) {
  ops.op_kill(pid, signo, apiName);
}

function kill(pid, signo = "SIGTERM") {
  opKill(pid, signo, "Deno.kill()");
}

function opRunStatus(rid) {
  return core.opAsync("op_run_status", rid);
}

function opRun(request) {
  assert(request.cmd.length > 0);
  return ops.op_run(request);
}

async function runStatus(rid) {
  const res = await opRunStatus(rid);

  if (res.gotSignal) {
    const signal = res.exitSignal;
    return { success: false, code: 128 + signal, signal };
  } else if (res.exitCode != 0) {
    return { success: false, code: res.exitCode };
  } else {
    return { success: true, code: 0 };
  }
}

class Process {
  constructor(res) {
    this.rid = res.rid;
    this.pid = res.pid;

    if (res.stdinRid && res.stdinRid > 0) {
      this.stdin = new FsFile(res.stdinRid);
    }

    if (res.stdoutRid && res.stdoutRid > 0) {
      this.stdout = new FsFile(res.stdoutRid);
    }

    if (res.stderrRid && res.stderrRid > 0) {
      this.stderr = new FsFile(res.stderrRid);
    }
  }

  status() {
    return runStatus(this.rid);
  }

  async output() {
    if (!this.stdout) {
      throw new TypeError("stdout was not piped");
    }
    try {
      return await readAll(this.stdout);
    } finally {
      this.stdout.close();
    }
  }

  async stderrOutput() {
    if (!this.stderr) {
      throw new TypeError("stderr was not piped");
    }
    try {
      return await readAll(this.stderr);
    } finally {
      this.stderr.close();
    }
  }

  close() {
    core.close(this.rid);
  }

  kill(signo = "SIGTERM") {
    opKill(this.pid, signo, "Deno.Process.kill()");
  }
}

function run({
  cmd,
  cwd = undefined,
  clearEnv = false,
  env = {},
  gid = undefined,
  uid = undefined,
  stdout = "inherit",
  stderr = "inherit",
  stdin = "inherit",
}) {
  if (cmd[0] != null) {
    cmd = [
      pathFromURL(cmd[0]),
      ...new SafeArrayIterator(ArrayPrototypeSlice(cmd, 1)),
    ];
  }
  const res = opRun({
    cmd: ArrayPrototypeMap(cmd, String),
    cwd,
    clearEnv,
    env: ObjectEntries(env),
    gid,
    uid,
    stdin,
    stdout,
    stderr,
  });
  return new Process(res);
}

const illegalConstructorKey = Symbol("illegalConstructorKey");

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
  ipc = -1,
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
    ipc,
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
  #waitPromise;
  #waitComplete = false;

  #pipeFd;
  // internal, used by ext/node
  get _pipeFd() {
    return this.#pipeFd;
  }

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

  #stdout = null;
  get stdout() {
    if (this.#stdout == null) {
      throw new TypeError("stdout is not piped");
    }
    return this.#stdout;
  }

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
    pipeFd, // internal
  } = null) {
    if (key !== illegalConstructorKey) {
      throw new TypeError("Illegal constructor.");
    }

    this.#rid = rid;
    this.#pid = pid;
    this.#pipeFd = pipeFd;

    if (stdinRid !== null) {
      this.#stdin = writableStreamForRid(stdinRid);
    }

    if (stdoutRid !== null) {
      this.#stdout = readableStreamForRidUnrefable(stdoutRid);
    }

    if (stderrRid !== null) {
      this.#stderr = readableStreamForRidUnrefable(stderrRid);
    }

    const onAbort = () => this.kill("SIGTERM");
    signal?.[abortSignal.add](onAbort);

    const waitPromise = core.opAsync("op_spawn_wait", this.#rid);
    this.#waitPromise = waitPromise;
    this.#status = PromisePrototypeThen(waitPromise, (res) => {
      signal?.[abortSignal.remove](onAbort);
      this.#waitComplete = true;
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
    if (this.#waitComplete) {
      throw new TypeError("Child process has already terminated.");
    }
    ops.op_spawn_kill(this.#rid, signo);
  }

  async [SymbolAsyncDispose]() {
    try {
      ops.op_spawn_kill(this.#rid, "SIGTERM");
    } catch {
      // ignore errors from killing the process (such as ESRCH or BadResource)
    }
    await this.#status;
  }

  ref() {
    core.refOpPromise(this.#waitPromise);
    if (this.#stdout) readableStreamForRidUnrefableRef(this.#stdout);
    if (this.#stderr) readableStreamForRidUnrefableRef(this.#stderr);
  }

  unref() {
    core.unrefOpPromise(this.#waitPromise);
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

export { ChildProcess, Command, kill, Process, run };
