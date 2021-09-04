// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  deferred,
  delay,
  unitTest,
} from "./test_util.ts";

unitTest(
  { ignore: Deno.build.os !== "windows" },
  function signalsNotImplemented() {
    assertThrows(
      () => {
        Deno.signal("SIGINT");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGALRM");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGCHLD");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGHUP");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGINT");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGIO");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGPIPE");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGQUIT");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGTERM");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGUSR1");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGUSR2");
      },
      Error,
      "not implemented",
    );
    assertThrows(
      () => {
        Deno.signal("SIGWINCH");
      },
      Error,
      "not implemented",
    );
  },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true, net: true } },
  async function signalStreamTest() {
    const resolvable = deferred();
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    let c = 0;
    const sig = Deno.signal("SIGUSR1");
    setTimeout(async () => {
      await delay(20);
      for (const _ of Array(3)) {
        // Sends SIGUSR1 3 times.
        Deno.kill(Deno.pid, "SIGUSR1");
        await delay(20);
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

// This tests that pending op_signal_poll doesn't block the runtime from exiting the process.
unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true, read: true } },
  async function signalStreamExitTest() {
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        "--unstable",
        "(async () => { for await (const _ of Deno.signal('SIGIO')) {} })()",
      ],
    });
    const res = await p.status();
    assertEquals(res.code, 0);
    p.close();
  },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true } },
  async function signalPromiseTest() {
    const resolvable = deferred();
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    const sig = Deno.signal("SIGUSR1");
    setTimeout(() => {
      Deno.kill(Deno.pid, "SIGUSR1");
      resolvable.resolve();
    }, 20);
    await sig;
    sig.dispose();

    clearInterval(t);
    await resolvable;
  },
);

// https://github.com/denoland/deno/issues/9806
unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true } },
  async function signalPromiseTest2() {
    const resolvable = deferred();
    // This prevents the program from exiting.
    const t = setInterval(() => {}, 1000);

    let called = false;
    const sig = Deno.signal("SIGUSR1");
    sig.then(() => {
      called = true;
    });
    setTimeout(() => {
      sig.dispose();
      setTimeout(() => {
        resolvable.resolve();
      }, 10);
    }, 10);

    clearInterval(t);
    await resolvable;

    // Promise callback is not called because it didn't get
    // the corresponding signal.
    assert(!called);
  },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { run: true } },
  function signalShorthandsTest() {
    let s: Deno.SignalStream;
    s = Deno.signal("SIGALRM");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGCHLD");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGHUP");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGINT");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGIO");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGPIPE");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGQUIT");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGTERM");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGUSR1");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGUSR2");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
    s = Deno.signal("SIGWINCH");
    assert(s instanceof Deno.SignalStream);
    s.dispose();
  },
);
