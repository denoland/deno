// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test(
  { permissions: { write: true, run: true, read: true } },
  async function spawnWithCwdIsAsync() {
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

    const child = Deno.spawn(Deno.execPath(), {
      cwd,
      args: ["run", "--allow-read", programFile],
    });

    // Write the expected exit code *after* starting deno.
    // This is how we verify that `Child` is actually asynchronous.
    const code = 84;
    Deno.writeFileSync(`${cwd}/${exitCodeFile}`, enc.encode(`${code}`));

    const status = await child.wait();
    await Deno.remove(cwd, { recursive: true });
    assertEquals(status.success, false);
    assertEquals(status.code, code);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnStdinPiped() {
    const child = Deno.spawn(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
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
  async function spawnStdoutPiped() {
    const child = Deno.spawn(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
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
  async function spawnStderrPiped() {
    const child = Deno.spawn(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('hello'))",
      ],
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
  { permissions: { run: true, write: true, read: true } },
  async function spawnRedirectStdoutStderr() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const file = await Deno.open(fileName, {
      create: true,
      write: true,
    });

    const child = Deno.spawn(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stderr.write(new TextEncoder().encode('error\\n')); Deno.stdout.write(new TextEncoder().encode('output\\n'));",
      ],
      stdout: "piped",
      stderr: "piped",
    });
    await child.stdout.pipeTo(file.writable, {
      preventClose: true,
    });
    await child.stderr.pipeTo(file.writable);
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
  async function spawnRedirectStdin() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const encoder = new TextEncoder();
    await Deno.writeFile(fileName, encoder.encode("hello"));
    const file = await Deno.open(fileName);

    const child = Deno.spawn(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
      stdin: "piped",
    });
    await file.readable.pipeTo(child.stdin, {
      preventClose: true,
    });

    const status = await child.wait();
    assertEquals(status.code, 0);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnKillSuccess() {
    const child = Deno.spawn(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
    });

    child.kill("SIGKILL");
    const status = await child.wait();

    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, undefined);
    } else {
      assertEquals(status.code, 137);
      assertEquals(status.signal, 9);
    }
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnKillFailed() {
    const child = Deno.spawn(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 5000)"],
    });

    assertThrows(() => {
      // @ts-expect-error testing runtime error of bad signal
      child.kill("foobar");
    }, TypeError);

    await child.wait();
  },
);

Deno.test(
  { permissions: { read: true, run: false } },
  async function commandPermissions() {
    await assertRejects(async () => {
      await Deno.command(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      });
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { read: true, run: false } },
  function commandSyncPermissions() {
    assertThrows(() => {
      Deno.commandSync(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      });
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandSuccess() {
    const { status } = await Deno.command(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
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
  function commandSyncSuccess() {
    const { status } = Deno.commandSync(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
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
  async function commandUrl() {
    const { status } = await Deno.command(
      new URL(`file:///${Deno.execPath()}`),
      {
        args: ["eval", "console.log('hello world')"],
      },
    );

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncUrl() {
    const { status } = Deno.commandSync(new URL(`file:///${Deno.execPath()}`), {
      args: ["eval", "console.log('hello world')"],
    });

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, undefined);
  },
);

Deno.test({ permissions: { run: true } }, async function commandNotFound() {
  await assertRejects(
    () => Deno.command("this file hopefully doesn't exist"),
    Deno.errors.NotFound,
  );
});

Deno.test({ permissions: { run: true } }, function commandSyncNotFound() {
  assertThrows(
    () => Deno.commandSync("this file hopefully doesn't exist"),
    Deno.errors.NotFound,
  );
});

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandFailedWithCode() {
    const { status } = await Deno.command(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    });
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncFailedWithCode() {
    const { status } = Deno.commandSync(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    });
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, undefined);
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  async function commandFailedWithSignal() {
    const { status } = await Deno.command(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    });
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

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  function commandSyncFailedWithSignal() {
    const { status } = Deno.commandSync(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    });
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

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandOutput() {
    const { stdout } = await Deno.command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
    });

    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "hello");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncOutput() {
    const { stdout } = Deno.commandSync(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
    });

    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "hello");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandStderrOutput() {
    const { stderr } = await Deno.command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('error'))",
      ],
    });

    const s = new TextDecoder().decode(stderr);
    assertEquals(s, "error");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncStderrOutput() {
    const { stderr } = Deno.commandSync(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('error'))",
      ],
    });

    const s = new TextDecoder().decode(stderr);
    assertEquals(s, "error");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandEnv() {
    const { stdout } = await Deno.command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stdout.write(new TextEncoder().encode(Deno.env.get('FOO') + Deno.env.get('BAR')))",
      ],
      env: {
        FOO: "0123",
        BAR: "4567",
      },
    });
    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "01234567");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandEnv() {
    const { stdout } = Deno.commandSync(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stdout.write(new TextEncoder().encode(Deno.env.get('FOO') + Deno.env.get('BAR')))",
      ],
      env: {
        FOO: "0123",
        BAR: "4567",
      },
    });
    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "01234567");
  },
);

Deno.test(
  { permissions: { run: true, read: true, env: true } },
  async function commandClearEnv() {
    const { stdout } = await Deno.command(Deno.execPath(), {
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

    const obj = JSON.parse(new TextDecoder().decode(stdout));

    // can't check for object equality because the OS may set additional env
    // vars for processes, so we check if PATH isn't present as that is a common
    // env var across OS's and isn't set for processes.
    assertEquals(obj.FOO, "23147");
    assert(!("PATH" in obj));
  },
);

Deno.test(
  { permissions: { run: true, read: true, env: true } },
  function commandSyncClearEnv() {
    const { stdout } = Deno.commandSync(Deno.execPath(), {
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

    const obj = JSON.parse(new TextDecoder().decode(stdout));

    // can't check for object equality because the OS may set additional env
    // vars for processes, so we check if PATH isn't present as that is a common
    // env var across OS's and isn't set for processes.
    assertEquals(obj.FOO, "23147");
    assert(!("PATH" in obj));
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
    ignore: Deno.build.os === "windows",
  },
  async function commandUid() {
    const { stdout } = await Deno.command("id", {
      args: ["-u"],
    });

    const currentUid = new TextDecoder().decode(stdout);

    if (currentUid !== "0") {
      await assertRejects(async () => {
        await Deno.command("echo", {
          args: ["fhqwhgads"],
          uid: 0,
        });
      }, Deno.errors.PermissionDenied);
    }
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
    ignore: Deno.build.os === "windows",
  },
  function commandSyncUid() {
    const { stdout } = Deno.commandSync("id", {
      args: ["-u"],
    });

    const currentUid = new TextDecoder().decode(stdout);

    if (currentUid !== "0") {
      assertThrows(() => {
        Deno.commandSync("echo", {
          args: ["fhqwhgads"],
          uid: 0,
        });
      }, Deno.errors.PermissionDenied);
    }
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
    ignore: Deno.build.os === "windows",
  },
  async function commandGid() {
    const { stdout } = await Deno.command("id", {
      args: ["-g"],
    });

    const currentGid = new TextDecoder().decode(stdout);

    if (currentGid !== "0") {
      await assertRejects(async () => {
        await Deno.command("echo", {
          args: ["fhqwhgads"],
          gid: 0,
        });
      }, Deno.errors.PermissionDenied);
    }
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
    ignore: Deno.build.os === "windows",
  },
  function commandSyncGid() {
    const { stdout } = Deno.commandSync("id", {
      args: ["-g"],
    });

    const currentGid = new TextDecoder().decode(stdout);

    if (currentGid !== "0") {
      assertThrows(() => {
        Deno.commandSync("echo", {
          args: ["fhqwhgads"],
          gid: 0,
        });
      }, Deno.errors.PermissionDenied);
    }
  },
);
