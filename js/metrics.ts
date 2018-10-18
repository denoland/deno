// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

interface Metrics {
  opsDispatched: number;
  opsCompleted: number;
  bytesSentControl: number;
  bytesSentData: number;
  bytesReceived: number;
}

/** Receive metrics from the privileged side of Deno. */
export function metrics(): Metrics {
  return res(dispatch.sendSync(...req()));
}

function req(): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = flatbuffers.createBuilder();
  msg.Metrics.startMetrics(builder);
  const inner = msg.Metrics.endMetrics(builder);
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
