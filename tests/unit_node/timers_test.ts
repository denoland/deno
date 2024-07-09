// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, fail } from "@std/assert/mod.ts";
import * as timers from "node:timers";
import * as timersPromises from "node:timers/promises";

Deno.test("[node/timers setTimeout]", () => {
  {
    const { clearTimeout, setTimeout } = timers;
    const id = setTimeout(() => {});
    clearTimeout(id);
  }

  {
    const id = timers.setTimeout(() => {});
    timers.clearTimeout(id);
  }
});

Deno.test("[node/timers setInterval]", () => {
  {
    const { clearInterval, setInterval } = timers;
    const id = setInterval(() => {});
    clearInterval(id);
  }

  {
    const id = timers.setInterval(() => {});
    timers.clearInterval(id);
  }
});

Deno.test("[node/timers setImmediate]", async () => {
  {
    const { clearImmediate, setImmediate } = timers;
    const imm = setImmediate(() => {});
    clearImmediate(imm);
  }

  {
    const imm = timers.setImmediate(() => {});
    timers.clearImmediate(imm);
  }

  {
    const deffered = Promise.withResolvers<void>();
    const imm = timers.setImmediate(
      (a, b) => {
        assert(a === 1);
        assert(b === 2);
        deffered.resolve();
      },
      1,
      2,
    );
    await deffered;
    timers.clearImmediate(imm);
  }
});

Deno.test("[node/timers/promises setTimeout]", () => {
  const { setTimeout } = timersPromises;
  const p = setTimeout(0);

  assert(p instanceof Promise);
  return p;
});

// Regression test for https://github.com/denoland/deno/issues/17981
Deno.test("[node/timers refresh cancelled timer]", () => {
  const { setTimeout, clearTimeout } = timers;
  const p = setTimeout(() => {
    fail();
  }, 1);
  clearTimeout(p);
  p.refresh();
});

Deno.test("[node/timers setImmediate returns Immediate object]", () => {
  const { clearImmediate, setImmediate } = timers;

  const imm = setImmediate(() => {});
  imm.unref();
  imm.ref();
  imm.hasRef();
  clearImmediate(imm);
});
