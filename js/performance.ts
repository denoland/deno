// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { sendSync } from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";

export class Performance {
  /** Returns a current UNIX timestamp
   *
   *       import { now } from "deno";
   *
   *       const unix = now();
   *       console.log(`${now} ms from UNIX epoch!`);
   */
  now(): number {
    const builder = flatbuffers.createBuilder();
    msg.Now.startNow(builder);
    const inner = msg.Now.endNow(builder);
    const baseRes = sendSync(builder, msg.Any.Now, inner)!;
    assert(msg.Any.NowRes === baseRes.innerType());
    const res = new msg.NowRes();
    assert(baseRes.inner(res) != null);
    return res.time().toFloat64();
  }
}
