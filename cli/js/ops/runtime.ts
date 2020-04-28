// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

// TODO(bartlomieju): these two types are duplicated
// in `cli/js/build.ts` - deduplicate
export type OperatingSystem = "mac" | "win" | "linux";
export type Arch = "x64" | "arm64";

export interface Start {
  arch: Arch;
  args: string[];
  cwd: string;
  debugFlag: boolean;
  denoVersion: string;
  location: string; // Absolute URL.
  noColor: boolean;
  os: OperatingSystem;
  pid: number;
  repl: boolean;
  tsVersion: string;
  unstableFlag: boolean;
  v8Version: string;
  versionFlag: boolean;
}

export function start(): Start {
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
