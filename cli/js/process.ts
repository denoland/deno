// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { File } from "./files.ts";
import { close } from "./ops/resources.ts";
import { Closer, Reader, Writer } from "./io.ts";
import { readAll } from "./buffer.ts";
import { kill, runStatus as runStatusOp, run as runOp } from "./ops/process.ts";

// TODO Maybe extend VSCode's 'CommandOptions'?
// See https://code.visualstudio.com/docs/editor/tasks-appendix#_schema-for-tasksjson
export interface RunOptions {
  cmd: string[];
  cwd?: string;
  env?: { [key: string]: string };
  stdout?: "inherit" | "piped" | "null" | number;
  stderr?: "inherit" | "piped" | "null" | number;
  stdin?: "inherit" | "piped" | "null" | number;
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

export class Process<T extends RunOptions = RunOptions> {
  readonly rid: number;
  readonly pid: number;
  readonly stdin!: T["stdin"] extends "piped" ? Writer & Closer : null;
  readonly stdout!: T["stdout"] extends "piped" ? Reader & Closer : null;
  readonly stderr!: T["stderr"] extends "piped" ? Reader & Closer : null;

  // @internal
  constructor(res: RunResponse) {
    this.rid = res.rid;
    this.pid = res.pid;

    if (res.stdinRid && res.stdinRid > 0) {
      this.stdin = (new File(res.stdinRid) as unknown) as Process<T>["stdin"];
    }

    if (res.stdoutRid && res.stdoutRid > 0) {
      this.stdout = (new File(res.stdoutRid) as unknown) as Process<
        T
      >["stdout"];
    }

    if (res.stderrRid && res.stderrRid > 0) {
      this.stderr = (new File(res.stderrRid) as unknown) as Process<
        T
      >["stderr"];
    }
  }

  status(): Promise<ProcessStatus> {
    return runStatus(this.rid);
  }

  async output(): Promise<Uint8Array> {
    if (!this.stdout) {
      throw new TypeError("stdout was not piped");
    }
    try {
      return await readAll(this.stdout as Reader & Closer);
    } finally {
      (this.stdout as Reader & Closer).close();
    }
  }

  async stderrOutput(): Promise<Uint8Array> {
    if (!this.stderr) {
      throw new TypeError("stderr was not piped");
    }
    try {
      return await readAll(this.stderr as Reader & Closer);
    } finally {
      (this.stderr as Reader & Closer).close();
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

export function run<T extends RunOptions = RunOptions>({
  cmd,
  cwd = undefined,
  env = {},
  stdout = "inherit",
  stderr = "inherit",
  stdin = "inherit",
}: T): Process<T> {
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
  return new Process<T>(res);
}
