// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

export interface Metrics {
  opsDispatched: number;
  opsCompleted: number;
  bytesSentControl: number;
  bytesSentData: number;
  bytesReceived: number;
}

const OP_METRICS = new JsonOp("metrics");

/** Receive metrics from the privileged side of Deno.
 *
 *      > console.table(Deno.metrics())
 *      ┌──────────────────┬────────┐
 *      │     (index)      │ Values │
 *      ├──────────────────┼────────┤
 *      │  opsDispatched   │   9    │
 *      │   opsCompleted   │   9    │
 *      │ bytesSentControl │  504   │
 *      │  bytesSentData   │   0    │
 *      │  bytesReceived   │  856   │
 *      └──────────────────┴────────┘
 */
export function metrics(): Metrics {
  return OP_METRICS.sendSync();
}
