// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";
import {
  readableStreamFromReader,
  writableStreamFromWriter,
} from "../../../test_util/std/streams/conversion.ts";

Deno.test(
  { permissions: { read: true, run: false } },
  async function runPermissions() {
    await assertRejects(() => {
      const cmd = new Deno.Command(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      });
      return cmd.output();
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runSuccess() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
    });
    const status = await cmd.status({
      stderr: "null",
      stdout: "null",
    });

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runUrl() {
    const cmd = new Deno.Command(new URL(`file:///${Deno.execPath()}`), {
      args: ["eval", "console.log('hello world')"],
    });

    const status = await cmd.status({
      stderr: "null",
      stdout: "null",
    });
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function runInvalidStdio() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
    });
    assertThrows(() => {
      cmd.spawn({
        // @ts-expect-error because should throw on invalid stdin.
        stdin: "a",
      });
    });
    assertThrows(() =>
      cmd.spawn({
        // @ts-expect-error because should throw on invalid stdout.
        stdout: "b",
      })
    );
    assertThrows(() =>
      cmd.spawn({
        // @ts-expect-error because should throw on invalid stderr.
        stderr: "c",
      })
    );
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runCommandFailedWithCode() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    });
    const status = await cmd.status();
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  async function runCommandFailedWithSignal() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    });
    const status = await cmd.status();
    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, undefined);
    } else {
      assertEquals(status.code, 128 + 9);
      assertEquals(status.signal, 9);
    }
  },
);

Deno.test({ permissions: { run: true } }, async function runNotFound() {
  let error;
  try {
    const cmd = new Deno.Command("this file hopefully doesn't exist");
    await cmd.status();
  } catch (e) {
    error = e;
  }
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

    const cmd = new Deno.Command(Deno.execPath(), {
      cwd,
      args: ["run", "--allow-read", programFile],
    });
    const child = cmd.spawn();

    // Write the expected exit code *after* starting deno.
    // This is how we verify that `Child` is actually asynchronous.
    const code = 84;
    Deno.writeFileSync(`${cwd}/${exitCodeFile}`, enc.encode(`${code}`));

    const status = await child.wait();
    assertEquals(status.success, false);
    assertEquals(status.code, code);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStdinPiped(): Promise<
    void
  > {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
    });
    const child = cmd.spawn({
      stdin: "piped",
    });

    assert(child.stdin !== null);
    assert(child.stdout === null);
    assert(child.stderr === null);

    const msg = new TextEncoder().encode("hello");
    const writer = child.stdin.getWriter();
    await writer.write(msg);
    writer.releaseLock();

    const status = await child.wait();
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStdoutPiped(): Promise<
    void
  > {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
    });
    const child = cmd.spawn({
      stdout: "piped",
    });

    assert(child.stdin === null);
    assert(child.stdout !== null);
    assert(child.stderr === null);

    const readable = child.stdout.pipeThrough(new TextDecoderStream());
    const reader = readable.getReader();
    const res = await reader.read();
    assert(!res.done);
    assertEquals(res.value, "hello");

    const resEnd = await reader.read();
    assert(resEnd.done);
    assertEquals(resEnd.value, undefined);
    reader.releaseLock();

    const status = await child.wait();
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStderrPiped(): Promise<
    void
  > {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('hello'))",
      ],
    });
    const child = cmd.spawn({
      stderr: "piped",
    });

    assert(child.stdin === null);
    assert(child.stdout === null);
    assert(child.stderr !== null);

    const readable = child.stderr.pipeThrough(new TextDecoderStream());
    const reader = readable.getReader();
    const res = await reader.read();
    assert(!res.done);
    assertEquals(res.value, "hello");

    const resEnd = await reader.read();
    assert(resEnd.done);
    assertEquals(resEnd.value, undefined);
    reader.releaseLock();

    const status = await child.wait();
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runOutput() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
    });

    const output = await cmd.output();
    const s = new TextDecoder().decode(output.stdout);
    assertEquals(s, "hello");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runStderrOutput(): Promise<
    void
  > {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('error'))",
      ],
    });

    const output = await cmd.output();
    const s = new TextDecoder().decode(output.stderr);
    assertEquals(s, "error");
  },
);

Deno.test(
  { permissions: { run: true, write: true, read: true } },
  async function runRedirectStdoutStderr() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const file = await Deno.open(fileName, {
      create: true,
      write: true,
    });
    const fileWriter = writableStreamFromWriter(file);

    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stderr.write(new TextEncoder().encode('error\\n')); Deno.stdout.write(new TextEncoder().encode('output\\n'));",
      ],
    });
    const child = cmd.spawn({
      stdout: "piped",
      stderr: "piped",
    });
    await child.stdout.pipeTo(fileWriter, {
      preventClose: true,
    });
    await child.stderr.pipeTo(fileWriter);
    await child.wait();

    const fileContents = await Deno.readFile(fileName);
    const decoder = new TextDecoder();
    const text = decoder.decode(fileContents);

    assertStringIncludes(text, "error");
    assertStringIncludes(text, "output");
  },
);

Deno.test(
  { permissions: { run: true, write: true, read: true } },
  async function runRedirectStdin() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const encoder = new TextEncoder();
    await Deno.writeFile(fileName, encoder.encode("hello"));
    const file = await Deno.open(fileName);
    const fileReader = readableStreamFromReader(file);

    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
    });
    const child = cmd.spawn({
      stdin: "piped",
    });
    await fileReader.pipeTo(child.stdin);

    const status = await child.wait();
    assertEquals(status.code, 0);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runEnv() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stdout.write(new TextEncoder().encode(Deno.env.get('FOO') + Deno.env.get('BAR')))",
      ],
      env: {
        FOO: "0123",
        BAR: "4567",
      },
    });

    const output = await cmd.output();
    const s = new TextDecoder().decode(output.stdout);
    assertEquals(s, "01234567");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function runKillAfterStatus() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log('hello')"],
    });
    const child = cmd.spawn();
    await child.wait();

    let error = null;
    try {
      child.kill("SIGTERM");
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
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function killSuccess() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
    });
    const child = cmd.spawn();

    Deno.kill(child.pid, "SIGINT");
    const status = await child.wait();

    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, undefined);
    } else {
      assertEquals(status.code, 130);
      assertEquals(status.signal, 2);
    }
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function killFailed() {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 5000)"],
    });
    const child = cmd.spawn();

    assertThrows(() => {
      // @ts-expect-error testing runtime error of bad signal
      Deno.kill(child.pid, "foobar");
    }, TypeError);

    await child.wait();
  },
);

Deno.test(
  { permissions: { run: true, read: true, env: true } },
  async function clearEnv(): Promise<void> {
    const cmd = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "-p",
        "JSON.stringify(Deno.env.toObject())",
      ],
      clearEnv: true,
      env: {
        FOO: "23147",
      },
    });

    const output = await cmd.output();
    const obj = JSON.parse(new TextDecoder().decode(output.stdout));

    // can't check for object equality because the OS may set additional env vars for processes
    // so we check if PATH isn't present as that is a common env var across OS's and isn't set for processes.
    assertEquals(obj.FOO, "23147");
    assert(!("PATH" in obj));
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
    ignore: Deno.build.os === "windows",
  },
  async function uid(): Promise<void> {
    const cmd = new Deno.Command("id", {
      args: ["-u"],
    });

    const output = await cmd.output();
    const currentUid = new TextDecoder().decode(output.stdout);

    if (currentUid !== "0") {
      await assertRejects(() => {
        const cmd = new Deno.Command("echo", {
          args: ["fhqwhgads"],
          uid: 0,
        });
        return cmd.status();
      }, Deno.errors.PermissionDenied);
    }
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
    ignore: Deno.build.os === "windows",
  },
  async function gid(): Promise<void> {
    const cmd = new Deno.Command("id", {
      args: ["-g"],
    });

    const output = await cmd.output();
    const currentGid = new TextDecoder().decode(output.stdout);

    if (currentGid !== "0") {
      await assertRejects(() => {
        const cmd = new Deno.Command("echo", {
          args: ["fhqwhgads"],
          gid: 0,
        });
        return cmd.status();
      }, Deno.errors.PermissionDenied);
    }
  },
);
