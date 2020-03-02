// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./dispatch_json.ts";

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
