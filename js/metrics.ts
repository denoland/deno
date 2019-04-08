// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

export interface Metrics {
  opsDispatched: number;
  opsCompleted: number;
  bytesSentControl: number;
  bytesSentData: number;
  bytesReceived: number;
}

function req(): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  const inner = msg.Metrics.createMetrics(builder);
  return [builder, msg.Any.Metrics, inner];
}

function res(baseRes: null | msg.Base): Metrics {
  assert(baseRes !== null);
  assert(msg.Any.MetricsRes === baseRes!.innerType());
  const res = new msg.MetricsRes();
  assert(baseRes!.inner(res) !== null);

  return {
    opsDispatched: res.opsDispatched().toFloat64(),
    opsCompleted: res.opsCompleted().toFloat64(),
    bytesSentControl: res.bytesSentControl().toFloat64(),
    bytesSentData: res.bytesSentData().toFloat64(),
    bytesReceived: res.bytesReceived().toFloat64()
  };
}

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
  return res(dispatch.sendSync(...req()));
}
