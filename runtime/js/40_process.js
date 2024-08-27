// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_kill,
  op_run,
  op_run_status,
  op_spawn_child,
  op_spawn_kill,
  op_spawn_sync,
  op_spawn_wait,
} from "ext:core/ops";
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
  SymbolFor,
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
  op_kill(pid, signo, apiName);
}

function kill(pid, signo = "SIGTERM") {
  opKill(pid, signo, "Deno.kill()");
}

function opRunStatus(rid) {
  return op_run_status(rid);
}

function opRun(request) {
  assert(request.cmd.length > 0);
  return op_run(request);
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
      this.stdin = new FsFile(res.stdinRid, SymbolFor("Deno.internal.FsFile"));
    }

    if (res.stdoutRid && res.stdoutRid > 0) {
      this.stdout = new FsFile(
        res.stdoutRid,
        SymbolFor("Deno.internal.FsFile"),
      );
    }

    if (res.stderrRid && res.stderrRid > 0) {
      this.stderr = new FsFile(
        res.stderrRid,
        SymbolFor("Deno.internal.FsFile"),
      );
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
  env = { __proto__: null },
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
  internals.warnOnDeprecatedApi(
    "Deno.run()",
    (new Error()).stack,
    `Use "Deno.Command()" API instead.`,
  );
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

export const kExtraStdio = Symbol("extraStdio");
export const kIpc = Symbol("ipc");
export const kDetached = Symbol("detached");

const illegalConstructorKey = Symbol("illegalConstructorKey");

function spawnChildInner(command, apiName, {
  args = [],
  cwd = undefined,
  clearEnv = false,
  env = { __proto__: null },
  uid = undefined,
  gid = undefined,
  signal = undefined,
  stdin = "null",
  stdout = "piped",
  stderr = "piped",
  windowsRawArguments = false,
  [kDetached]: detached = false,
  [kExtraStdio]: extraStdio = [],
  [kIpc]: ipc = -1,
} = { __proto__: null }) {
  const child = op_spawn_child({
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
    extraStdio,
    detached,
  }, apiName);
  return new ChildProcess(illegalConstructorKey, {
    ...child,
    signal,
  });
}

function spawnChild(command, options = { __proto__: null }) {
  return spawnChildInner(
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

const _ipcPipeRid = Symbol("[[ipcPipeRid]]");
const _extraPipeRids = Symbol("[[_extraPipeRids]]");

internals.getIpcPipeRid = (process) => process[_ipcPipeRid];
internals.getExtraPipeRids = (process) => process[_extraPipeRids];

class ChildProcess {
  #rid;
  #waitPromise;
  #waitComplete = false;

  [_ipcPipeRid];
  [_extraPipeRids];

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
    ipcPipeRid, // internal
    extraPipeRids,
  } = null) {
    if (key !== illegalConstructorKey) {
      throw new TypeError("Illegal constructor.");
    }

    this.#rid = rid;
    this.#pid = pid;
    this[_ipcPipeRid] = ipcPipeRid;
    this[_extraPipeRids] = extraPipeRids;

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

    const waitPromise = op_spawn_wait(this.#rid);
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
    op_spawn_kill(this.#rid, signo);
  }

  async [SymbolAsyncDispose]() {
    try {
      op_spawn_kill(this.#rid, "SIGTERM");
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
  env = { __proto__: null },
  uid = undefined,
  gid = undefined,
  stdin = "null",
  stdout = "piped",
  stderr = "piped",
  windowsRawArguments = false,
} = { __proto__: null }) {
  if (stdin === "piped") {
    throw new TypeError(
      "Piped stdin is not supported for this function, use 'Deno.Command().spawn()' instead",
    );
  }
  const result = op_spawn_sync({
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
    extraStdio: [],
    detached: false,
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
