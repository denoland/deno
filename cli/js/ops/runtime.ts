// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

// TODO(bartlomieju): these two types are duplicated
// in `cli/js/build.ts` - deduplicate
export type OperatingSystem = "mac" | "win" | "linux";
export type Arch = "x64" | "arm64";

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
  os: OperatingSystem;
  arch: Arch;
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

/** Receive metrics from the privileged side of Deno.
 *
 *      > console.table(Deno.metrics())
 *      ┌─────────────────────────┬────────┐
 *      │         (index)         │ Values │
 *      ├─────────────────────────┼────────┤
 *      │      opsDispatched      │   3    │
 *      │    opsDispatchedSync    │   2    │
 *      │   opsDispatchedAsync    │   1    │
 *      │ opsDispatchedAsyncUnref │   0    │
 *      │      opsCompleted       │   3    │
 *      │    opsCompletedSync     │   2    │
 *      │    opsCompletedAsync    │   1    │
 *      │ opsCompletedAsyncUnref  │   0    │
 *      │    bytesSentControl     │   73   │
 *      │      bytesSentData      │   0    │
 *      │      bytesReceived      │  375   │
 *      └─────────────────────────┴────────┘
 */
export function metrics(): Metrics {
  return sendSync("op_metrics");
}
