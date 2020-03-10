// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { now as opNow } from "./ops/timers.ts";

export class Performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  now(): number {
    const res = opNow();
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }
}
