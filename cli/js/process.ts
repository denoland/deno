// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./ops/dispatch_json.ts";
import { File } from "./files.ts";
import { close } from "./ops/resources.ts";
import { ReadCloser, WriteCloser } from "./io.ts";
import { readAll } from "./buffer.ts";
import { assert, unreachable } from "./util.ts";
import { build } from "./build.ts";

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

interface RunStatusResponse {
  gotSignal: boolean;
  exitCode: number;
  exitSignal: number;
}

async function runStatus(rid: number): Promise<ProcessStatus> {
  const res = (await sendAsync("op_run_status", {
    rid
  })) as RunStatusResponse;

  if (res.gotSignal) {
    const signal = res.exitSignal;
    return { signal, success: false };
  } else {
    const code = res.exitCode;
    return { code, success: code === 0 };
  }
}

/** Send a signal to process under given PID. Unix only at this moment.
 * If pid is negative, the signal will be sent to the process group identified
 * by -pid.
 * Requires the `--allow-run` flag.
 */
export function kill(pid: number, signo: number): void {
  sendSync("op_kill", { pid, signo });
}

export class Process {
  readonly rid: number;
  readonly pid: number;
  readonly stdin?: WriteCloser;
  readonly stdout?: ReadCloser;
  readonly stderr?: ReadCloser;

  // @internal
  constructor(res: RunResponse) {
    this.rid = res.rid;
    this.pid = res.pid;

    if (res.stdinRid && res.stdinRid > 0) {
      this.stdin = new File(res.stdinRid);
    }

    if (res.stdoutRid && res.stdoutRid > 0) {
      this.stdout = new File(res.stdoutRid);
    }

    if (res.stderrRid && res.stderrRid > 0) {
      this.stderr = new File(res.stderrRid);
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

// TODO: this method is only used to validate proper option, probably can be renamed
function stdioMap(s: string): string {
  switch (s) {
    case "inherit":
    case "piped":
    case "null":
      return s;
    default:
      return unreachable();
  }
}

function isRid(arg: unknown): arg is number {
  return !isNaN(arg as number);
}

interface RunResponse {
  rid: number;
  pid: number;
  stdinRid: number | null;
  stdoutRid: number | null;
  stderrRid: number | null;
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
  assert(opt.args.length > 0);
  let env: Array<[string, string]> = [];
  if (opt.env) {
    env = Array.from(Object.entries(opt.env));
  }

  let stdin = stdioMap("inherit");
  let stdout = stdioMap("inherit");
  let stderr = stdioMap("inherit");
  let stdinRid = 0;
  let stdoutRid = 0;
  let stderrRid = 0;

  if (opt.stdin) {
    if (isRid(opt.stdin)) {
      stdinRid = opt.stdin;
    } else {
      stdin = stdioMap(opt.stdin);
    }
  }

  if (opt.stdout) {
    if (isRid(opt.stdout)) {
      stdoutRid = opt.stdout;
    } else {
      stdout = stdioMap(opt.stdout);
    }
  }

  if (opt.stderr) {
    if (isRid(opt.stderr)) {
      stderrRid = opt.stderr;
    } else {
      stderr = stdioMap(opt.stderr);
    }
  }

  const req = {
    args: opt.args.map(String),
    cwd: opt.cwd,
    env,
    stdin,
    stdout,
    stderr,
    stdinRid,
    stdoutRid,
    stderrRid
  };

  const res = sendSync("op_run", req) as RunResponse;
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
export const Signal: { [key: string]: number } = {};

export function setSignals(): void {
  if (build.os === "mac") {
    Object.assign(Signal, MacOSSignal);
  } else {
    Object.assign(Signal, LinuxSignal);
  }
}
