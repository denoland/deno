// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { sendSync, msg, flatbuffers } from "./dispatch_flatbuffers";
import { assert } from "./util";

export class Performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  now(): number {
    const builder = flatbuffers.createBuilder();
    const inner = msg.Now.createNow(builder);
    const baseRes = sendSync(builder, msg.Any.Now, inner)!;
    assert(msg.Any.NowRes === baseRes.innerType());
    const res = new msg.NowRes();
    assert(baseRes.inner(res) != null);
    return res.seconds().toFloat64() * 1e3 + res.subsecNanos() / 1e6;
  }
}
