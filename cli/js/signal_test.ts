// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assertEquals, assertThrows } from "./test_util.ts";

function defer(n: number): Promise<void> {
  return new Promise((resolve, _) => {
    setTimeout(resolve, n);
  });
}

if (Deno.build.os === "win") {
  test(async function signalsNotImplemented(): Promise<void> {
    assertThrows(
      () => {
        Deno.signal(1);
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.alarm(); // for SIGALRM
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.child(); // for SIGCHLD
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.hungup(); // for SIGHUP
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.interrupt(); // for SIGINT
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.io(); // for SIGIO
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.pipe(); // for SIGPIPE
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.quit(); // for SIGQUIT
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.terminate(); // for SIGTERM
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.userDefined1(); // for SIGUSR1
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.userDefined2(); // for SIGURS2
      },
      Error,
      "not implemented"
    );
    assertThrows(
      () => {
        Deno.signals.windowChange(); // for SIGWINCH
      },
      Error,
      "not implemented"
    );
  });
} else {
  test(function emptySignalTest(): void {
    assertThrows(
      () => {
        // @ts-ignore
        Deno.signal();
      },
      Error,
      "No signals are given."
    );
  });

  testPerm({ run: true }, async function singleSignalTest(): Promise<void> {
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
    });

    for await (const _ of sig) {
      c += 1;
    }

    assertEquals(c, 3);

    clearTimeout(t);
  });

  testPerm({ run: true }, async function multipleSignalTest(): Promise<void> {
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    let c = 0;
    const sig = Deno.signal(
      Deno.Signal.SIGUSR1,
      Deno.Signal.SIGUSR2,
      Deno.Signal.SIGINT
    );

    setTimeout(async () => {
      await defer(20);
      Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
      await defer(20);
      Deno.kill(Deno.pid, Deno.Signal.SIGUSR2);
      await defer(20);
      Deno.kill(Deno.pid, Deno.Signal.SIGINT);
      await defer(20);
      Deno.kill(Deno.pid, Deno.Signal.SIGUSR2);
      await defer(20);
      Deno.kill(Deno.pid, Deno.Signal.SIGINT);
      await defer(20);
      Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
      await defer(20);
      sig.dispose();
    });

    for await (const _ of sig) {
      c += 1;
    }

    assertEquals(c, 6);

    clearTimeout(t);
  });

  testPerm({ run: true }, async function signalPromiseTest(): Promise<void> {
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    const sig = Deno.signal(Deno.Signal.SIGUSR1);
    setTimeout(() => {
      Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
    }, 20);
    await sig;
    sig.dispose();

    clearTimeout(t);
  });

  testPerm({ run: true }, async function signalShorthandsTest(): Promise<void> {
    let s: Deno.Signals;
    s = Deno.signals.alarm(); // for SIGALRM
    s.dispose();
    s = Deno.signals.child(); // for SIGCHLD
    s.dispose();
    s = Deno.signals.hungup(); // for SIGHUP
    s.dispose();
    s = Deno.signals.interrupt(); // for SIGINT
    s.dispose();
    s = Deno.signals.io(); // for SIGIO
    s.dispose();
    s = Deno.signals.pipe(); // for SIGPIPE
    s.dispose();
    s = Deno.signals.quit(); // for SIGQUIT
    s.dispose();
    s = Deno.signals.terminate(); // for SIGTERM
    s.dispose();
    s = Deno.signals.userDefined1(); // for SIGUSR1
    s.dispose();
    s = Deno.signals.userDefined2(); // for SIGURS2
    s.dispose();
    s = Deno.signals.windowChange(); // for SIGWINCH
    s.dispose();
  });
}
