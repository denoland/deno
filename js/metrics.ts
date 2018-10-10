// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

interface Metrics {
  opsDispatched: number;
  opsCompleted: number;
  controlBytesSent: number;
  dataBytesSent: number;
  bytesReceived: number;
}

export function metrics(): Metrics {
  return res(dispatch.sendSync(...req()));
}

function req(): [flatbuffers.Builder, msg.Any, flatbuffers.Offset] {
  const builder = new flatbuffers.Builder();
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
    controlBytesSent: res.controlBytesSent().toFloat64(),
    dataBytesSent: res.dataBytesSent().toFloat64(),
    bytesReceived: res.bytesReceived().toFloat64()
  };
}
