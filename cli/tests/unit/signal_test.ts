// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  deferred,
  unitTest,
} from "./test_util.ts";

function defer(n: number): Promise<void> {
  return new Promise((resolve: () => void, _) => {
    setTimeout(resolve, n);
  });
}

unitTest(
  { ignore: Deno.build.os !== "windows" },
  function signalsNotImplemented(): void {
    assertThrows(
      () => {
        Deno.signal(1);
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.alarm(); // for SIGALRM
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.child(); // for SIGCHLD
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.hungup(); // for SIGHUP
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.io(); // for SIGIO
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.pipe(); // for SIGPIPE
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.quit(); // for SIGQUIT
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.terminate(); // for SIGTERM
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.userDefined1(); // for SIGUSR1
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.userDefined2(); // for SIGURS2
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
    assertThrows(
      () => {
        Deno.signals.windowChange(); // for SIGWINCH
      },
      Error,
      "Windows only supports ctrl-c(SIGINT)!",
    );
  },
);

unitTest(
    { ignore: Deno.build.os !== "windows", perms: { run: true, net: true }},
    async function signalWindowsStreamTest(): Promise<void> {
        const resolvable = deferred();
        // This prevents the program from exiting.
        const t = setInterval(() => {}, 1000);

        let c = 0;
        const sig = Deno.signal(Deno.Signal.SIGINT);
        setTimeout(async () => {
            await defer(20);
            for (const _ of Array(3)) {
                // Sends SIGUSR1 3 times.
                Deno.kill(Deno.pid, Deno.Signal.SIGINT);
                await defer(20);
            }
            sig.dispose();
            resolvable.resolve();
        });

        for await (const _ of sig) {
            c += 1;
        }

        assertEquals(c, 3);

        clearInterval(t);
        await resolvable;
    },
);

unitTest(
    { ignore: Deno.build.os !== "windows", perms: { run: true } },
    async function signalWindowsPromiseTest(): Promise<void> {
        const resolvable = deferred();
        // This prevents the program from exiting.
        const t = setInterval(() => {}, 1000);

        const sig = Deno.signal(Deno.Signal.SIGINT);
        setTimeout(() => {
            Deno.kill(Deno.pid, Deno.Signal.SIGINT);
            resolvable.resolve();
        }, 20);
        await sig;
        sig.dispose();

        clearInterval(t);
        await resolvable;
    },
);

unitTest(
    { ignore: Deno.build.os !== "windows", perms: { run: true } },
    function signalWindowsShorthandsTest(): void {
        const s: Deno.SignalStream = Deno.signals.interrupt(); // for SIGINT
        assert(s instanceof Deno.SignalStream);
        s.dispose();
    },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true, net: true } },
  async function signalStreamTest(): Promise<void> {
    const resolvable = deferred();
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    let c = 0;
    const sig = Deno.signal(Deno.Signal.SIGUSR1);
    setTimeout(async () => {
      await defer(20);
      for (const _ of Array(3)) {
        // Sends SIGUSR1 3 times.
        Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
        await defer(20);
      }
      sig.dispose();
      resolvable.resolve();
    });

    for await (const _ of sig) {
      c += 1;
    }

    assertEquals(c, 3);

    clearInterval(t);
    await resolvable;
  },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true } },
  async function signalPromiseTest(): Promise<void> {
    const resolvable = deferred();
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    const sig = Deno.signal(Deno.Signal.SIGUSR1);
    setTimeout(() => {
      Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
      resolvable.resolve();
    }, 20);
    await sig;
    sig.dispose();

    clearInterval(t);
    await resolvable;
  },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true } },
  function signalShorthandsTest(): void {
    let s: Deno.SignalStream;
    s = Deno.signals.alarm(); // for SIGALRM
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.child(); // for SIGCHLD
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.hungup(); // for SIGHUP
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.interrupt(); // for SIGINT
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.io(); // for SIGIO
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.pipe(); // for SIGPIPE
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.quit(); // for SIGQUIT
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.terminate(); // for SIGTERM
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.userDefined1(); // for SIGUSR1
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.userDefined2(); // for SIGURS2
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signals.windowChange(); // for SIGWINCH
    assert(s instanceof Deno.SignalStream);
    s.dispose();
  },
);
