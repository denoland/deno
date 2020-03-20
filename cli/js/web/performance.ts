// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { now as opNow } from "../ops/timers.ts";

export class Performance {
  now(): number {
    const res = opNow();
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }
}
