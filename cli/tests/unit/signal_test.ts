// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, deferred, delay } from "./test_util.ts";

Deno.test(
  { ignore: Deno.build.os !== "windows" },
  function signalsNotImplemented() {
    assertThrows(
      () => {
        Deno.addSignalListener("SIGINT", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGALRM", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGCHLD", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGHUP", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGINT", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGIO", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGPIPE", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGQUIT", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGTERM", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGUSR1", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGUSR2", () => {});
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.addSignalListener("SIGWINCH", () => {});
      },
      Error,
      "not implemented",
    );
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true },
  },
  async function signalListenerTest() {
    const resolvable = deferred();
    let c = 0;
    const listener = () => {
      c += 1;
    };
    Deno.addSignalListener("SIGUSR1", listener);
    setTimeout(async () => {
      // Sends SIGUSR1 3 times.
      for (const _ of Array(3)) {
        await delay(20);
        Deno.kill(Deno.pid, "SIGUSR1");
      }
      await delay(20);
      Deno.removeSignalListener("SIGUSR1", listener);
      resolvable.resolve();
    });

    await resolvable;
    assertEquals(c, 3);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true },
  },
  async function multipleSignalListenerTest() {
    const resolvable = deferred();
    let c = "";
    const listener0 = () => {
      c += "0";
    };
    const listener1 = () => {
      c += "1";
    };
    Deno.addSignalListener("SIGUSR2", listener0);
    Deno.addSignalListener("SIGUSR2", listener1);
    setTimeout(async () => {
      // Sends SIGUSR2 3 times.
      for (const _ of Array(3)) {
        await delay(20);
        Deno.kill(Deno.pid, "SIGUSR2");
      }
      await delay(20);
      Deno.removeSignalListener("SIGUSR2", listener1);
      // Sends SIGUSR2 3 times.
      for (const _ of Array(3)) {
        await delay(20);
        Deno.kill(Deno.pid, "SIGUSR2");
      }
      await delay(20);
      // Sends SIGUSR1 (irrelevant signal) 3 times.
      for (const _ of Array(3)) {
        await delay(20);
        Deno.kill(Deno.pid, "SIGUSR1");
      }
      await delay(20);
      Deno.removeSignalListener("SIGUSR2", listener0);
      resolvable.resolve();
    });

    await resolvable;
    // The first 3 events are handled by both handlers
    // The last 3 events are handled only by handler0
    assertEquals(c, "010101000");
  },
);

// This tests that pending op_signal_poll doesn't block the runtime from exiting the process.
Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true, read: true },
  },
  async function canExitWhileListeningToSignal() {
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        "--unstable",
        "Deno.addSignalListener('SIGIO', () => {})",
      ],
    });
    const res = await p.status();
    assertEquals(res.code, 0);
    p.close();
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { run: true },
  },
  function signalInvalidHandlerTest() {
    assertThrows(() => {
      // deno-lint-ignore no-explicit-any
      Deno.addSignalListener("SIGINT", "handler" as any);
    });
    assertThrows(() => {
      // deno-lint-ignore no-explicit-any
      Deno.removeSignalListener("SIGINT", "handler" as any);
    });
  },
);

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
