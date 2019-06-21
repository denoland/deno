// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/cli/msg_generated";

import { File, close } from "./files";
import { ReadCloser, WriteCloser } from "./io";
import { readAll } from "./buffer";
import { assert, unreachable } from "./util";
import { platform } from "./build";

/** How to handle subprocess stdio.
 *
 * "inherit" The default if unspecified. The child inherits from the
 * corresponding parent descriptor.
 *
 * "piped"  A new pipe should be arranged to connect the parent and child
 * subprocesses.
 *
 * "null" This stream will be ignored. This is the equivalent of attaching the
 * stream to /dev/null.
 */
export type ProcessStdio = "inherit" | "piped" | "null";

// TODO Maybe extend VSCode's 'CommandOptions'?
// See https://code.visualstudio.com/docs/editor/tasks-appendix#_schema-for-tasksjson
export interface RunOptions {
  args: string[];
  cwd?: string;
  env?: { [key: string]: string };
  stdout?: ProcessStdio | number;
  stderr?: ProcessStdio | number;
  stdin?: ProcessStdio | number;
}

async function runStatus(rid: number): Promise<ProcessStatus> {
  const builder = flatbuffers.createBuilder();
  const inner = msg.RunStatus.createRunStatus(builder, rid);

  const baseRes = await dispatch.sendAsync(builder, msg.Any.RunStatus, inner);
  assert(baseRes != null);
  assert(msg.Any.RunStatusRes === baseRes!.innerType());
  const res = new msg.RunStatusRes();
  assert(baseRes!.inner(res) != null);

  if (res.gotSignal()) {
    const signal = res.exitSignal();
    return { signal, success: false };
  } else {
    const code = res.exitCode();
    return { code, success: code === 0 };
  }
}

/** Send a signal to process under given PID. Unix only at this moment.
 * If pid is negative, the signal will be sent to the process group identified
 * by -pid.
 */
export function kill(pid: number, signo: number): void {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Kill.createKill(builder, pid, signo);
  dispatch.sendSync(builder, msg.Any.Kill, inner);
}

export class Process {
  readonly rid: number;
  readonly pid: number;
  readonly stdin?: WriteCloser;
  readonly stdout?: ReadCloser;
  readonly stderr?: ReadCloser;

  // @internal
  constructor(res: msg.RunRes) {
    this.rid = res.rid();
    this.pid = res.pid();

    if (res.stdinRid() > 0) {
      this.stdin = new File(res.stdinRid());
    }

    if (res.stdoutRid() > 0) {
      this.stdout = new File(res.stdoutRid());
    }

    if (res.stderrRid() > 0) {
      this.stderr = new File(res.stderrRid());
    }
  }

  async status(): Promise<ProcessStatus> {
    return await runStatus(this.rid);
  }

  /** Buffer the stdout and return it as Uint8Array after EOF.
   * You must set stdout to "piped" when creating the process.
   * This calls close() on stdout after its done.
   */
  async output(): Promise<Uint8Array> {
    if (!this.stdout) {
      throw new Error("Process.output: stdout is undefined");
    }
    try {
      return await readAll(this.stdout);
    } finally {
      this.stdout.close();
    }
  }

  /** Buffer the stderr and return it as Uint8Array after EOF.
   * You must set stderr to "piped" when creating the process.
   * This calls close() on stderr after its done.
   */
  async stderrOutput(): Promise<Uint8Array> {
    if (!this.stderr) {
      throw new Error("Process.stderrOutput: stderr is undefined");
    }
    try {
      return await readAll(this.stderr);
    } finally {
      this.stderr.close();
    }
  }

  close(): void {
    close(this.rid);
  }

  kill(signo: number): void {
    kill(this.pid, signo);
  }
}

export interface ProcessStatus {
  success: boolean;
  code?: number;
  signal?: number; // TODO: Make this a string, e.g. 'SIGTERM'.
}

function stdioMap(s: ProcessStdio): msg.ProcessStdio {
  switch (s) {
    case "inherit":
      return msg.ProcessStdio.Inherit;
    case "piped":
      return msg.ProcessStdio.Piped;
    case "null":
      return msg.ProcessStdio.Null;
    default:
      return unreachable();
  }
}

function isRid(arg: unknown): arg is number {
  return !isNaN(arg as number);
}

/**
 * Spawns new subprocess.
 *
 * Subprocess uses same working directory as parent process unless `opt.cwd`
 * is specified.
 *
 * Environmental variables for subprocess can be specified using `opt.env`
 * mapping.
 *
 * By default subprocess inherits stdio of parent process. To change that
 * `opt.stdout`, `opt.stderr` and `opt.stdin` can be specified independently -
 * they can be set to either `ProcessStdio` or `rid` of open file.
 */
export function run(opt: RunOptions): Process {
  const builder = flatbuffers.createBuilder();
  const argsOffset = msg.Run.createArgsVector(
    builder,
    opt.args.map((a): number => builder.createString(a))
  );
  const cwdOffset = opt.cwd == null ? 0 : builder.createString(opt.cwd);
  const kvOffset: flatbuffers.Offset[] = [];
  if (opt.env) {
    for (const [key, val] of Object.entries(opt.env)) {
      const keyOffset = builder.createString(key);
      const valOffset = builder.createString(String(val));
      kvOffset.push(msg.KeyValue.createKeyValue(builder, keyOffset, valOffset));
    }
  }
  const envOffset = msg.Run.createEnvVector(builder, kvOffset);

  let stdInOffset = stdioMap("inherit");
  let stdOutOffset = stdioMap("inherit");
  let stdErrOffset = stdioMap("inherit");
  let stdinRidOffset = 0;
  let stdoutRidOffset = 0;
  let stderrRidOffset = 0;

  if (opt.stdin) {
    if (isRid(opt.stdin)) {
      stdinRidOffset = opt.stdin;
    } else {
      stdInOffset = stdioMap(opt.stdin);
    }
  }

  if (opt.stdout) {
    if (isRid(opt.stdout)) {
      stdoutRidOffset = opt.stdout;
    } else {
      stdOutOffset = stdioMap(opt.stdout);
    }
  }

  if (opt.stderr) {
    if (isRid(opt.stderr)) {
      stderrRidOffset = opt.stderr;
    } else {
      stdErrOffset = stdioMap(opt.stderr);
    }
  }

  const inner = msg.Run.createRun(
    builder,
    argsOffset,
    cwdOffset,
    envOffset,
    stdInOffset,
    stdOutOffset,
    stdErrOffset,
    stdinRidOffset,
    stdoutRidOffset,
    stderrRidOffset
  );
  const baseRes = dispatch.sendSync(builder, msg.Any.Run, inner);
  assert(baseRes != null);
  assert(msg.Any.RunRes === baseRes!.innerType());
  const res = new msg.RunRes();
  assert(baseRes!.inner(res) != null);

  return new Process(res);
}

// From `kill -l`
enum LinuxSignal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGBUS = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGUSR1 = 10,
  SIGSEGV = 11,
  SIGUSR2 = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGSTKFLT = 16,
  SIGCHLD = 17,
  SIGCONT = 18,
  SIGSTOP = 19,
  SIGTSTP = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGURG = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGIO = 29,
  SIGPWR = 30,
  SIGSYS = 31
}

// From `kill -l`
enum MacOSSignal {
  SIGHUP = 1,
  SIGINT = 2,
  SIGQUIT = 3,
  SIGILL = 4,
  SIGTRAP = 5,
  SIGABRT = 6,
  SIGEMT = 7,
  SIGFPE = 8,
  SIGKILL = 9,
  SIGBUS = 10,
  SIGSEGV = 11,
  SIGSYS = 12,
  SIGPIPE = 13,
  SIGALRM = 14,
  SIGTERM = 15,
  SIGURG = 16,
  SIGSTOP = 17,
  SIGTSTP = 18,
  SIGCONT = 19,
  SIGCHLD = 20,
  SIGTTIN = 21,
  SIGTTOU = 22,
  SIGIO = 23,
  SIGXCPU = 24,
  SIGXFSZ = 25,
  SIGVTALRM = 26,
  SIGPROF = 27,
  SIGWINCH = 28,
  SIGINFO = 29,
  SIGUSR1 = 30,
  SIGUSR2 = 31
}

/** Signals numbers. This is platform dependent.
 */
export const Signal = platform.os === "mac" ? MacOSSignal : LinuxSignal;
