// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync, sendAsync } from "./dispatch_json.ts";
import { assert } from "../util.ts";
import { build } from "../build.ts";

export function kill(pid: number, signo: number): void {
  if (build.os === "win") {
    throw new Error("Not yet implemented");
  }
  sendSync("op_kill", { pid, signo });
}

interface RunStatusResponse {
  gotSignal: boolean;
  exitCode: number;
  exitSignal: number;
}

export function runStatus(rid: number): Promise<RunStatusResponse> {
  return sendAsync("op_run_status", { rid });
}

interface RunRequest {
  cmd: string[];
  cwd?: string;
  env?: Array<[string, string]>;
  stdin: string;
  stdout: string;
  stderr: string;
  stdinRid: number;
  stdoutRid: number;
  stderrRid: number;
}

interface RunResponse {
  rid: number;
  pid: number;
  stdinRid: number | null;
  stdoutRid: number | null;
  stderrRid: number | null;
}

export function run(request: RunRequest): RunResponse {
  assert(request.cmd.length > 0);
  return sendSync("op_run", request);
}
