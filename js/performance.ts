// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { JsonOp } from "./dispatch_json.ts";

interface NowResponse {
  seconds: number;
  subsecNanos: number;
}

const OP_NOW = new JsonOp("now");

export class Performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  now(): number {
    const res = OP_NOW.sendSync() as NowResponse;
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }
}
