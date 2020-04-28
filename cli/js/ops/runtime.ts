// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export interface Start {
  cwd: string;
  pid: number;
  args: string[];
  location: string; // Absolute URL.
  repl: boolean;
  debugFlag: boolean;
  depsFlag: boolean;
  typesFlag: boolean;
  versionFlag: boolean;
  denoVersion: string;
  v8Version: string;
  tsVersion: string;
  noColor: boolean;
  target: string;
}

export function opStart(): Start {
  return sendSync("op_start");
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
  return sendSync("op_metrics");
}
