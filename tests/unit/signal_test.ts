// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, delay } from "./test_util.ts";

Deno.test(
  { ignore: Deno.build.os !== "windows" },
  function signalsNotImplemented() {
    const msg =
      "Windows only supports ctrl-c (SIGINT) and ctrl-break (SIGBREAK).";
    assertThrows(
      () => {
        Deno.addSignalListener("SIGALRM", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGCHLD", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGHUP", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGIO", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGPIPE", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGQUIT", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGTERM", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGUSR1", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGUSR2", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGWINCH", () => {});
      },
      Error,
      msg,
    );
    assertThrows(
      () => Deno.addSignalListener("SIGKILL", () => {}),
      Error,
      msg,
    );
    assertThrows(
      () => Deno.addSignalListener("SIGSTOP", () => {}),
      Error,
      msg,
    );
    assertThrows(
      () => Deno.addSignalListener("SIGILL", () => {}),
      Error,
      msg,
    );
    assertThrows(
      () => Deno.addSignalListener("SIGFPE", () => {}),
      Error,
      msg,
    );
    assertThrows(
      () => Deno.addSignalListener("SIGSEGV", () => {}),
      Error,
      msg,
    );
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true },
  },
  async function signalListenerTest() {
    let c = 0;
    const listener = () => {
      c += 1;
    };
    // This test needs to be careful that it doesn't accidentally aggregate multiple
    // signals into one. Sending two or more SIGxxx before the handler can be run will
    // result in signal coalescing.
    Deno.addSignalListener("SIGUSR1", listener);
    // Sends SIGUSR1 3 times.
    for (let i = 1; i <= 3; i++) {
      await delay(1);
      Deno.kill(Deno.pid, "SIGUSR1");
      while (c < i) {
        await delay(20);
      }
    }
    Deno.removeSignalListener("SIGUSR1", listener);
    await delay(100);
    assertEquals(c, 3);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true },
  },
  async function multipleSignalListenerTest() {
    let c = "";
    const listener0 = () => {
      c += "0";
    };
    const listener1 = () => {
      c += "1";
    };
    // This test needs to be careful that it doesn't accidentally aggregate multiple
    // signals into one. Sending two or more SIGxxx before the handler can be run will
    // result in signal coalescing.
    Deno.addSignalListener("SIGUSR2", listener0);
    Deno.addSignalListener("SIGUSR2", listener1);

    // Sends SIGUSR2 3 times.
    for (let i = 1; i <= 3; i++) {
      await delay(1);
      Deno.kill(Deno.pid, "SIGUSR2");
      while (c.length < i * 2) {
        await delay(20);
      }
    }

    Deno.removeSignalListener("SIGUSR2", listener1);

    // Sends SIGUSR2 3 times.
    for (let i = 1; i <= 3; i++) {
      await delay(1);
      Deno.kill(Deno.pid, "SIGUSR2");
      while (c.length < 6 + i) {
        await delay(20);
      }
    }

    // Sends SIGUSR1 (irrelevant signal) 3 times.
    // By default SIGUSR1 terminates, so set it to a no-op for this test.
    let count = 0;
    const irrelevant = () => {
      count++;
    };
    Deno.addSignalListener("SIGUSR1", irrelevant);
    for (const _ of Array(3)) {
      await delay(20);
      Deno.kill(Deno.pid, "SIGUSR1");
    }
    while (count < 3) {
      await delay(20);
    }
    Deno.removeSignalListener("SIGUSR1", irrelevant);

    // No change
    assertEquals(c, "010101000");

    Deno.removeSignalListener("SIGUSR2", listener0);

    await delay(100);

    // The first 3 events are handled by both handlers
    // The last 3 events are handled only by handler0
    assertEquals(c, "010101000");
  },
);

// This tests that pending op_signal_poll doesn't block the runtime from exiting the process.
Deno.test(
  {
    permissions: { run: true, read: true },
  },
  async function canExitWhileListeningToSignal() {
    const { code } = await new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.addSignalListener('SIGINT', () => {})",
      ],
    }).output();
    assertEquals(code, 0);
  },
);

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { run: true },
  },
  function windowsThrowsOnNegativeProcessIdTest() {
    assertThrows(
      () => {
        Deno.kill(-1, "SIGKILL");
      },
      TypeError,
      "Invalid pid",
    );
  },
);

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { run: true },
  },
  function noOpenSystemIdleProcessTest() {
    let signal: Deno.Signal = "SIGKILL";

    assertThrows(
      () => {
        Deno.kill(0, signal);
      },
      TypeError,
      `Invalid pid`,
    );

    signal = "SIGTERM";
    assertThrows(
      () => {
        Deno.kill(0, signal);
      },
      TypeError,
      `Invalid pid`,
    );
  },
);

Deno.test(function signalInvalidHandlerTest() {
  assertThrows(() => {
    // deno-lint-ignore no-explicit-any
    Deno.addSignalListener("SIGINT", "handler" as any);
  });
  assertThrows(() => {
    // deno-lint-ignore no-explicit-any
    Deno.removeSignalListener("SIGINT", "handler" as any);
  });
});

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true },
  },
  function signalForbiddenSignalTest() {
    assertThrows(
      () => Deno.addSignalListener("SIGKILL", () => {}),
      TypeError,
      "Binding to signal 'SIGKILL' is not allowed",
    );
    assertThrows(
      () => Deno.addSignalListener("SIGSTOP", () => {}),
      TypeError,
      "Binding to signal 'SIGSTOP' is not allowed",
    );
    assertThrows(
      () => Deno.addSignalListener("SIGILL", () => {}),
      TypeError,
      "Binding to signal 'SIGILL' is not allowed",
    );
    assertThrows(
      () => Deno.addSignalListener("SIGFPE", () => {}),
      TypeError,
      "Binding to signal 'SIGFPE' is not allowed",
    );
    assertThrows(
      () => Deno.addSignalListener("SIGSEGV", () => {}),
      TypeError,
      "Binding to signal 'SIGSEGV' is not allowed",
    );
  },
);

Deno.test(
  { ignore: Deno.build.os !== "linux" },
  function signalAliasLinux() {
    const i = () => {};
    Deno.addSignalListener("SIGUNUSED", i);
    Deno.addSignalListener("SIGPOLL", i);

    Deno.removeSignalListener("SIGUNUSED", i);
    Deno.removeSignalListener("SIGPOLL", i);
  },
);
