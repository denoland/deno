// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

interface Metrics {
    opsExecuted: number;
    bytesRecv: number;
    bytesSent: number;
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
    const opsExecuted = res.opsExecuted();
    const bytesSent = res.bytesSent();
    const bytesRecv = res.bytesRecv();
    return { opsExecuted, bytesRecv, bytesSent };
}
