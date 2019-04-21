// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/cli/msg_generated";

import { File, close } from "./files";
import { ReadCloser, WriteCloser } from "./io";
import { readAll } from "./buffer";
import { assert, unreachable } from "./util";

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
  stdout?: ProcessStdio;
  stderr?: ProcessStdio;
  stdin?: ProcessStdio;
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
 * `opt.stdout`, `opt.stderr` and `opt.stdin` can be specified independently.
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
  const inner = msg.Run.createRun(
    builder,
    argsOffset,
    cwdOffset,
    envOffset,
    opt.stdin ? stdioMap(opt.stdin) : stdioMap("inherit"),
    opt.stdout ? stdioMap(opt.stdout) : stdioMap("inherit"),
    opt.stderr ? stdioMap(opt.stderr) : stdioMap("inherit")
  );
  const baseRes = dispatch.sendSync(builder, msg.Any.Run, inner);
  assert(baseRes != null);
  assert(msg.Any.RunRes === baseRes!.innerType());
  const res = new msg.RunRes();
  assert(baseRes!.inner(res) != null);

  return new Process(res);
}
