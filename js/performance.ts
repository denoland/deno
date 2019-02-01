// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/msg_generated";
import { sendSync } from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { assert } from "./util";

export class Performance {
  private denoStarted = 0;

  constructor () {
    this.denoStarted = new Date().getTime();
  }

  /** Returns a current time from Deno's start
   *
   *       const start = performance.now();
   *       someFunction();
   *       const end = performance.now();
   *       console.log(`someFunction ran for ${end - start} ms!`);
   */
  now(): number {
    const builder = flatbuffers.createBuilder();
    msg.Now.startNow(builder);
    const inner = msg.Now.endNow(builder);
    const baseRes = sendSync(builder, msg.Any.Now, inner)!;
    assert(msg.Any.NowRes === baseRes.innerType());
    const res = new msg.NowRes();
    assert(baseRes.inner(res) != null);
    return res.time().toFloat64() - this.denoStarted;
  }
}
