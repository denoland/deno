// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { core } from "../core.ts";

export interface Start {
  args: string[];
  cwd: string;
  debugFlag: boolean;
  denoVersion: string;
  noColor: boolean;
  pid: number;
  repl: boolean;
  target: string;
  tsVersion: string;
  unstableFlag: boolean;
  v8Version: string;
  versionFlag: boolean;
}

export function opStart(): Start {
  return core.dispatchJson.sendSync("op_start");
}

export function opMainModule(): string {
  return core.dispatchJson.sendSync("op_main_module");
}

export interface Metrics {
  opsDispatched: number;
  opsDispatchedSync: number;
  opsDispatchedAsync: number;
  opsDispatchedAsyncUnref: number;
  opsCompleted: number;
  opsCompletedSync: number;
  opsCompletedAsync: number;
  opsCompletedAsyncUnref: number;
  bytesSentControl: number;
  bytesSentData: number;
  bytesReceived: number;
}

export function metrics(): Metrics {
  return core.dispatchJson.sendSync("op_metrics");
}
