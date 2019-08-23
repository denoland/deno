// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as dispatch from "./dispatch";
import { sendSync } from "./dispatch_json";

export class Performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  now(): number {
    const res = sendSync(dispatch.OP_NOW);
    return res.seconds().toFloat64() * 1e3 + res.subsecNanos() / 1e6;
  }
}
