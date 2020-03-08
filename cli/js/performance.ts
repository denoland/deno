// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { sendSync } from "./ops/dispatch_json.ts";

interface NowResponse {
  seconds: number;
  subsecNanos: number;
}

export class Performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  now(): number {
    const res = sendSync("op_now") as NowResponse;
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }
}
