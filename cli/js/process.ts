// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { File } from "./files.ts";
import { close } from "./ops/resources.ts";
import { ReadCloser, WriteCloser } from "./io.ts";
import { readAll } from "./buffer.ts";
import { kill, runStatus as runStatusOp, run as runOp } from "./ops/process.ts";

export type ProcessStdio = "inherit" | "piped" | "null";

// TODO Maybe extend VSCode's 'CommandOptions'?
// See https://code.visualstudio.com/docs/editor/tasks-appendix#_schema-for-tasksjson
export interface RunOptions {
  cmd: string[];
  cwd?: string;
  env?: { [key: string]: string };
  stdout?: ProcessStdio | number;
  stderr?: ProcessStdio | number;
  stdin?: ProcessStdio | number;
}

async function runStatus(rid: number): Promise<ProcessStatus> {
  const res = await runStatusOp(rid);

  if (res.gotSignal) {
    const signal = res.exitSignal;
    return { signal, success: false };
  } else {
    const code = res.exitCode;
    return { code, success: code === 0 };
  }
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

  status(): Promise<ProcessStatus> {
    return runStatus(this.rid);
  }

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
export function run({
  cmd,
  cwd = undefined,
  env = {},
  stdout = "inherit",
  stderr = "inherit",
  stdin = "inherit",
}: RunOptions): Process {
  const res = runOp({
    cmd: cmd.map(String),
    cwd,
    env: Object.entries(env),
    stdin: isRid(stdin) ? "" : stdin,
    stdout: isRid(stdout) ? "" : stdout,
    stderr: isRid(stderr) ? "" : stderr,
    stdinRid: isRid(stdin) ? stdin : 0,
    stdoutRid: isRid(stdout) ? stdout : 0,
    stderrRid: isRid(stderr) ? stderr : 0,
  }) as RunResponse;
  return new Process(res);
}
