// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test("runSuccess", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
    stdout: "piped",
    stderr: "null",
  });
  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.stdout.close();
  p.close();
});

Deno.test("runUrl", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [
      new URL(`file:///${Deno.execPath()}`),
      "eval",
      "console.log('hello world')",
    ],
    stdout: "piped",
    stderr: "null",
  });
  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.stdout.close();
  p.close();
});

Deno.test("runStdinRid0", async function (): Promise<
  void
> {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
    stdin: 0,
    stdout: "piped",
    stderr: "null",
  });
  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.stdout.close();
  p.close();
});

Deno.test("runInvalidStdio", function (): void {
  assertThrows(() =>
    Deno.run({
      cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
      // @ts-expect-error because Deno.run should throw on invalid stdin.
      stdin: "a",
    })
  );
  assertThrows(() =>
    Deno.run({
      cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
      // @ts-expect-error because Deno.run should throw on invalid stdout.
      stdout: "b",
    })
  );
  assertThrows(() =>
    Deno.run({
      cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
      // @ts-expect-error because Deno.run should throw on invalid stderr.
      stderr: "c",
    })
  );
});

Deno.test("runCommandFailedWithCode", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "Deno.exit(41 + 1)"],
  });
  const status = await p.status();
  assertEquals(status.success, false);
  assertEquals(status.code, 42);
  assertEquals(status.signal, undefined);
  p.close();
});

Deno.test({
  name: "runCommandFailedWithSignal",
  ignore: Deno.build.os === "windows",
  async fn(): Promise<void> {
    const p = Deno.run({
      cmd: [Deno.execPath(), "eval", "--unstable", "Deno.kill(Deno.pid, 9)"],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, 128 + 9);
    assertEquals(status.signal, 9);
    p.close();
  },
});

Deno.test("runNotFound", function (): void {
  let error;
  try {
    Deno.run({ cmd: ["this file hopefully doesn't exist"] });
  } catch (e) {
    error = e;
  }
  assert(error !== undefined);
  assert(error instanceof Deno.errors.NotFound);
});

Deno.test("runWithCwdIsAsync", async function (): Promise<void> {
  const enc = new TextEncoder();
  const cwd = await Deno.makeTempDir({ prefix: "deno_command_test" });

  const exitCodeFile = "deno_was_here";
  const programFile = "poll_exit.ts";
  const program = `
async function tryExit() {
  try {
    const code = parseInt(await Deno.readTextFile("${exitCodeFile}"));
    Deno.exit(code);
  } catch {
    // Retry if we got here before deno wrote the file.
    setTimeout(tryExit, 0.01);
  }
}

tryExit();
`;

  Deno.writeFileSync(`${cwd}/${programFile}`, enc.encode(program));
  const p = Deno.run({
    cwd,
    cmd: [Deno.execPath(), "run", "--allow-read", programFile],
  });

  // Write the expected exit code *after* starting deno.
  // This is how we verify that `run()` is actually asynchronous.
  const code = 84;
  Deno.writeFileSync(`${cwd}/${exitCodeFile}`, enc.encode(`${code}`));

  const status = await p.status();
  assertEquals(status.success, false);
  assertEquals(status.code, code);
  assertEquals(status.signal, undefined);
  p.close();
});

Deno.test("runStdinPiped", async function (): Promise<
  void
> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
    ],
    stdin: "piped",
  });
  assert(p.stdin);
  assert(!p.stdout);
  assert(!p.stderr);

  const msg = new TextEncoder().encode("hello");
  const n = await p.stdin.write(msg);
  assertEquals(n, msg.byteLength);

  p.stdin.close();

  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

Deno.test("runStdoutPiped", async function (): Promise<
  void
> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "await Deno.stdout.write(new TextEncoder().encode('hello'))",
    ],
    stdout: "piped",
  });
  assert(!p.stdin);
  assert(!p.stderr);

  const data = new Uint8Array(10);
  let r = await p.stdout.read(data);
  if (r === null) {
    throw new Error("p.stdout.read(...) should not be null");
  }
  assertEquals(r, 5);
  const s = new TextDecoder().decode(data.subarray(0, r));
  assertEquals(s, "hello");
  r = await p.stdout.read(data);
  assertEquals(r, null);
  p.stdout.close();

  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

Deno.test("runStderrPiped", async function (): Promise<
  void
> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "await Deno.stderr.write(new TextEncoder().encode('hello'))",
    ],
    stderr: "piped",
  });
  assert(!p.stdin);
  assert(!p.stdout);

  const data = new Uint8Array(10);
  let r = await p.stderr.read(data);
  if (r === null) {
    throw new Error("p.stderr.read should not return null here");
  }
  assertEquals(r, 5);
  const s = new TextDecoder().decode(data.subarray(0, r));
  assertEquals(s, "hello");
  r = await p.stderr.read(data);
  assertEquals(r, null);
  p.stderr!.close();

  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

Deno.test("runOutput", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "await Deno.stdout.write(new TextEncoder().encode('hello'))",
    ],
    stdout: "piped",
  });
  const output = await p.output();
  const s = new TextDecoder().decode(output);
  assertEquals(s, "hello");
  p.close();
});

Deno.test("runStderrOutput", async function (): Promise<
  void
> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "await Deno.stderr.write(new TextEncoder().encode('error'))",
    ],
    stderr: "piped",
  });
  const error = await p.stderrOutput();
  const s = new TextDecoder().decode(error);
  assertEquals(s, "error");
  p.close();
});

Deno.test("runRedirectStdoutStderr", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const fileName = tempDir + "/redirected_stdio.txt";
  const file = await Deno.open(fileName, {
    create: true,
    write: true,
  });

  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "Deno.stderr.write(new TextEncoder().encode('error\\n')); Deno.stdout.write(new TextEncoder().encode('output\\n'));",
    ],
    stdout: file.rid,
    stderr: file.rid,
  });

  await p.status();
  p.close();
  file.close();

  const fileContents = await Deno.readFile(fileName);
  const decoder = new TextDecoder();
  const text = decoder.decode(fileContents);

  assertStringIncludes(text, "error");
  assertStringIncludes(text, "output");
});

Deno.test("runRedirectStdin", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const fileName = tempDir + "/redirected_stdio.txt";
  const encoder = new TextEncoder();
  await Deno.writeFile(fileName, encoder.encode("hello"));
  const file = await Deno.open(fileName);

  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
    ],
    stdin: file.rid,
  });

  const status = await p.status();
  assertEquals(status.code, 0);
  p.close();
  file.close();
});

Deno.test("runEnv", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "Deno.stdout.write(new TextEncoder().encode(Deno.env.get('FOO') + Deno.env.get('BAR')))",
    ],
    env: {
      FOO: "0123",
      BAR: "4567",
    },
    stdout: "piped",
  });
  const output = await p.output();
  const s = new TextDecoder().decode(output);
  assertEquals(s, "01234567");
  p.close();
});

Deno.test("runClose", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "setTimeout(() => Deno.stdout.write(new TextEncoder().encode('error')), 10000)",
    ],
    stderr: "piped",
  });
  assert(!p.stdin);
  assert(!p.stdout);

  p.close();

  const data = new Uint8Array(10);
  const r = await p.stderr.read(data);
  assertEquals(r, null);
  p.stderr.close();
});

Deno.test("runKillAfterStatus", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", 'console.log("hello")'],
  });
  await p.status();

  let error = null;
  try {
    p.kill(Deno.Signal.SIGTERM);
  } catch (e) {
    error = e;
  }

  assert(
    error instanceof Deno.errors.NotFound ||
      // On Windows, the underlying Windows API may return
      // `ERROR_ACCESS_DENIED` when the process has exited, but hasn't been
      // completely cleaned up yet and its `pid` is still valid.
      (Deno.build.os === "windows" &&
        error instanceof Deno.errors.PermissionDenied),
  );

  p.close();
});

Deno.test("signalNumbers", function (): void {
  if (Deno.build.os === "darwin") {
    assertEquals(Deno.Signal.SIGSTOP, 17);
  } else if (Deno.build.os === "linux") {
    assertEquals(Deno.Signal.SIGSTOP, 19);
  }
});

Deno.test("killSuccess", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "setTimeout(() => {}, 10000)"],
  });

  assertEquals(Deno.Signal.SIGINT, 2);
  Deno.kill(p.pid, Deno.Signal.SIGINT);
  const status = await p.status();

  assertEquals(status.success, false);
  try {
    assertEquals(status.code, 128 + Deno.Signal.SIGINT);
    assertEquals(status.signal, Deno.Signal.SIGINT);
  } catch {
    // TODO(nayeemrmn): On Windows sometimes the following values are given
    // instead. Investigate and remove this catch when fixed.
    assertEquals(status.code, 1);
    assertEquals(status.signal, undefined);
  }
  p.close();
});

Deno.test("killFailed", function (): void {
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "setTimeout(() => {}, 10000)"],
  });
  assert(!p.stdin);
  assert(!p.stdout);

  assertThrows(() => {
    Deno.kill(p.pid, 12345);
  }, TypeError);

  p.close();
});

Deno.test("killPermissions", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "run" });

  assertThrows(() => {
    // Unlike the other test cases, we don't have permission to spawn a
    // subprocess we can safely kill. Instead we send SIGCONT to the current
    // process - assuming that Deno does not have a special handler set for it
    // and will just continue even if a signal is erroneously sent.
    Deno.kill(Deno.pid, Deno.Signal.SIGCONT);
  }, Deno.errors.PermissionDenied);
});

Deno.test("runPermissions", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "run" });

  assertThrows(() => {
    Deno.run({
      cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
    });
  }, Deno.errors.PermissionDenied);
});
