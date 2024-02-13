// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows } from "../assert/mod.ts";
import { delay } from "../async/delay.ts";
import { signal } from "./mod.ts";

const isWindows = Deno.build.os === "windows";

Deno.test({
  name: "signal() throws when called with empty signals",
  ignore: isWindows,
  fn() {
    assertThrows(
      () => {
        // deno-lint-ignore no-explicit-any
        (signal as any)();
      },
      Error,
      "No signals are given. You need to specify at least one signal to create a signal stream.",
    );
  },
});

Deno.test({
  name: "signal() iterates for multiple signals",
  ignore: isWindows,
  fn: async () => {
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    let c = 0;
    const sig = signal(
      "SIGUSR1",
      "SIGUSR2",
    );

    setTimeout(async () => {
      await delay(20);
      Deno.kill(Deno.pid, "SIGUSR2");
      await delay(20);
      Deno.kill(Deno.pid, "SIGUSR1");
      await delay(20);
      Deno.kill(Deno.pid, "SIGUSR2");
      await delay(20);
      Deno.kill(Deno.pid, "SIGUSR1");
      await delay(20);
      sig.dispose();
    });

    for await (const _ of sig) {
      console.log(c);
      c += 1;
    }

    assertEquals(c, 4);

    clearTimeout(t);
  },
});

Deno.test({
  name: "signal(), multiple .next() results don't resolve at the same time",
  ignore: isWindows,
  async fn() {
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);
    const sig = signal("SIGUSR1");
    const sigIter = sig[Symbol.asyncIterator]();
    let done0 = false;
    let done1 = false;
    sigIter.next().then(() => {
      done0 = true;
    });
    sigIter.next().then(() => {
      done1 = true;
    });
    Deno.kill(Deno.pid, "SIGUSR1");
    await delay(20);
    assert(done0);
    assert(!done1);
    Deno.kill(Deno.pid, "SIGUSR1");
    await delay(20);
    assert(done0);
    assert(done1);
    sig.dispose();
    clearTimeout(t);
  },
});
