// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import * as msg from "gen/msg_generated";

import { File, close } from "./files";
import { ReadCloser, WriteCloser } from "./io";
import { readAll } from "./buffer";
import { assert, unreachable } from "./util";

/** How to handle subsubprocess stdio.
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
// tslint:disable-next-line:max-line-length
// See https://code.visualstudio.com/docs/editor/tasks-appendix#_schema-for-tasksjson
export interface RunOptions {
  args: string[];
  cwd?: string;
  stdout?: ProcessStdio;
  stderr?: ProcessStdio;
  stdin?: ProcessStdio;
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
   * You must have set stdout to "piped" in when creating the process.
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

export function run(opt: RunOptions): Process {
  const builder = flatbuffers.createBuilder();
  const argsOffset = msg.Run.createArgsVector(
    builder,
    opt.args.map(a => builder.createString(a))
  );
  const cwdOffset = opt.cwd == null ? -1 : builder.createString(opt.cwd);
  msg.Run.startRun(builder);
  msg.Run.addArgs(builder, argsOffset);
  if (opt.cwd != null) {
    msg.Run.addCwd(builder, cwdOffset);
  }
  if (opt.stdin) {
    msg.Run.addStdin(builder, stdioMap(opt.stdin!));
  }
  if (opt.stdout) {
    msg.Run.addStdout(builder, stdioMap(opt.stdout!));
  }
  if (opt.stderr) {
    msg.Run.addStderr(builder, stdioMap(opt.stderr!));
  }
  const inner = msg.Run.endRun(builder);
  const baseRes = dispatch.sendSync(builder, msg.Any.Run, inner);
  assert(baseRes != null);
  assert(msg.Any.RunRes === baseRes!.innerType());
  const res = new msg.RunRes();
  assert(baseRes!.inner(res) != null);

  return new Process(res);
}

async function runStatus(rid: number): Promise<ProcessStatus> {
  const builder = flatbuffers.createBuilder();
  msg.RunStatus.startRunStatus(builder);
  msg.RunStatus.addRid(builder, rid);
  const inner = msg.RunStatus.endRunStatus(builder);

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
