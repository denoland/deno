// deno-lint-ignore-file no-deprecated-deno-api
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test(
  { permissions: { read: true, run: false } },
  function runPermissions() {
    assertThrows(() => {
      // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
      Deno.run({
        cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
      });
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runSuccess() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      // freeze the array to ensure it's not modified
      cmd: Object.freeze([
        Deno.execPath(),
        "eval",
        "console.log('hello world')",
      ]),
      stdout: "piped",
      stderr: "null",
    });
    const status = await p.status();
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
    p.stdout.close();
    p.close();
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runUrl() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStdinRid0(): Promise<
    void
  > {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function runInvalidStdio() {
    assertThrows(() =>
      // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
      Deno.run({
        cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
        stdin: "a",
      })
    );
    assertThrows(() =>
      // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
      Deno.run({
        cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
        stdout: "b",
      })
    );
    assertThrows(() =>
      // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
      Deno.run({
        cmd: [Deno.execPath(), "eval", "console.log('hello world')"],
        stderr: "c",
      })
    );
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runCommandFailedWithCode() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [Deno.execPath(), "eval", "Deno.exit(41 + 1)"],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, undefined);
    p.close();
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  async function runCommandFailedWithSignal() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        "Deno.kill(Deno.pid, 'SIGKILL')",
      ],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, undefined);
    } else {
      assertEquals(status.code, 128 + 9);
      assertEquals(status.signal, 9);
    }
    p.close();
  },
);

Deno.test({ permissions: { run: true } }, function runNotFound() {
  let error;
  try {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    Deno.run({ cmd: ["this file hopefully doesn't exist"] });
  } catch (e) {
    error = e;
  }
  assert(error !== undefined);
  assert(error instanceof Deno.errors.NotFound);
});

Deno.test(
  { permissions: { write: true, run: true, read: true } },
  async function runWithCwdIsAsync() {
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
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStdinPiped(): Promise<
    void
  > {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        `
        const buffer = new Uint8Array(5);
        await Deno.stdin.read(buffer);
        if (new TextDecoder().decode(buffer) !== "hello") {
          throw new Error('Expected \\'hello\\'')
        }
        `,
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStdoutPiped(): Promise<
    void
  > {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStderrPiped(): Promise<
    void
  > {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runOutput() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStderrOutput(): Promise<
    void
  > {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  {
    permissions: { run: true, write: true, read: true },
  },
  async function runRedirectStdoutStderr() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    using file = await Deno.open(fileName, {
      create: true,
      write: true,
    });

    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        "Deno.stderr.write(new TextEncoder().encode('error\\n')); Deno.stdout.write(new TextEncoder().encode('output\\n'));",
      ],
      stdout: "piped",
      stderr: "piped",
    });

    await p.stdout.readable.pipeTo(file.writable, { preventClose: true });
    await p.stderr.readable.pipeTo(file.writable);
    await p.status();
    p.close();

    const fileContents = await Deno.readFile(fileName);
    const decoder = new TextDecoder();
    const text = decoder.decode(fileContents);

    assertStringIncludes(text, "error");
    assertStringIncludes(text, "output");
    // deno-lint-ignore no-console
    console.log("finished tgis test");
  },
);

Deno.test(
  {
    permissions: { run: true, write: true, read: true },
  },
  async function runRedirectStdin() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    await Deno.writeTextFile(fileName, "hello");
    using file = await Deno.open(fileName);

    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        `
        const buffer = new Uint8Array(5);
        await Deno.stdin.read(buffer);
        if (new TextDecoder().decode(buffer) !== "hello") {
          throw new Error('Expected \\'hello\\'')
        }
        `,
      ],
      stdin: "piped",
    });

    await file.readable.pipeTo(p.stdin.writable);
    const status = await p.status();
    assertEquals(status.code, 0);
    p.close();
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runEnv() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runClose() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runKillAfterStatus() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [Deno.execPath(), "eval", 'console.log("hello")'],
    });
    await p.status();

    let error = null;
    try {
      p.kill("SIGTERM");
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
  },
);

Deno.test({ permissions: { run: false } }, function killPermissions() {
  assertThrows(() => {
    // Unlike the other test cases, we don't have permission to spawn a
    // subprocess we can safely kill. Instead we send SIGCONT to the current
    // process - assuming that Deno does not have a special handler set for it
    // and will just continue even if a signal is erroneously sent.
    Deno.kill(Deno.pid, "SIGCONT");
  }, Deno.errors.NotCapable);
});

Deno.test(
  { ignore: Deno.build.os !== "windows", permissions: { run: true } },
  function negativePidInvalidWindows() {
    assertThrows(() => {
      Deno.kill(-1, "SIGTERM");
    }, TypeError);
  },
);

Deno.test(
  { ignore: Deno.build.os !== "windows", permissions: { run: true } },
  function invalidSignalNameWindows() {
    assertThrows(() => {
      Deno.kill(Deno.pid, "SIGUSR1");
    }, TypeError);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function killSuccess() {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [Deno.execPath(), "eval", "setTimeout(() => {}, 10000)"],
    });

    try {
      Deno.kill(p.pid, "SIGKILL");
      const status = await p.status();

      assertEquals(status.success, false);
      if (Deno.build.os === "windows") {
        assertEquals(status.code, 1);
        assertEquals(status.signal, undefined);
      } else {
        assertEquals(status.code, 137);
        assertEquals(status.signal, 9);
      }
    } finally {
      p.close();
    }
  },
);

Deno.test({ permissions: { run: true, read: true } }, function killFailed() {
  // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
  const p = Deno.run({
    cmd: [Deno.execPath(), "eval", "setTimeout(() => {}, 10000)"],
  });
  assert(!p.stdin);
  assert(!p.stdout);

  assertThrows(() => {
    // @ts-expect-error testing runtime error of bad signal
    Deno.kill(p.pid, "foobar");
  }, TypeError);

  p.close();
});

Deno.test(
  {
    permissions: { run: true, read: true, write: true },
    ignore: Deno.build.os === "windows",
  },
  async function non_existent_cwd(): Promise<void> {
    // @ts-ignore `Deno.run()` was soft-removed in Deno 2.
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "eval",
        `const dir = Deno.makeTempDirSync();
        Deno.chdir(dir);
        Deno.removeSync(dir);
        const p = Deno.run({cmd:[Deno.execPath(), "eval", "console.log(1);"]});
        const { code } = await p.status();
        p.close();
        Deno.exit(code);
        `,
      ],
      stdout: "piped",
      stderr: "piped",
    });

    const { code } = await p.status();
    const stderr = new TextDecoder().decode(await p.stderrOutput());
    p.close();
    p.stdout.close();
    assertStrictEquals(code, 1);
    assertStringIncludes(stderr, "failed resolving cwd:");
  },
);
