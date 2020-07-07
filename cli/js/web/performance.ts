// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { now as nowOp } from "../ops/timers.ts";

export class Performance {
  now(): number {
    const res = nowOp();
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }
}
