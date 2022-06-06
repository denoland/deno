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

    const child = Deno.spawnChild(Deno.execPath(), {
      cwd,
      args: ["run", "--allow-read", programFile],
      stdout: "inherit",
      stderr: "inherit",
    });

    // Write the expected exit code *after* starting deno.
    // This is how we verify that `Child` is actually asynchronous.
    const code = 84;
    Deno.writeFileSync(`${cwd}/${exitCodeFile}`, enc.encode(`${code}`));

    const status = await child.status;
    await Deno.remove(cwd, { recursive: true });
    assertEquals(status.success, false);
    assertEquals(status.code, code);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnStdinPiped() {
    const child = Deno.spawnChild(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
      stdin: "piped",
      stdout: "null",
      stderr: "null",
    });

    assert(child.stdin !== null);
    assert(child.stdout === null);
    assert(child.stderr === null);

    const msg = new TextEncoder().encode("hello");
    const writer = child.stdin.getWriter();
    await writer.write(msg);
    writer.releaseLock();

    await child.stdin.close();
    const status = await child.status;
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnStdoutPiped() {
    const child = Deno.spawnChild(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
      stderr: "null",
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

    const status = await child.status;
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnStderrPiped() {
    const child = Deno.spawnChild(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('hello'))",
      ],
      stderr: "piped",
      stdout: "null",
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

    const status = await child.status;
    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
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

    const child = Deno.spawnChild(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stderr.write(new TextEncoder().encode('error\\n')); Deno.stdout.write(new TextEncoder().encode('output\\n'));",
      ],
    });
    await child.stdout.pipeTo(file.writable, {
      preventClose: true,
    });
    await child.stderr.pipeTo(file.writable);
    await child.status;

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

    const child = Deno.spawnChild(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
      stdin: "piped",
      stdout: "null",
      stderr: "null",
    });
    await file.readable.pipeTo(child.stdin, {
      preventClose: true,
    });

    await child.stdin.close();
    const status = await child.status;
    assertEquals(status.code, 0);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnKillSuccess() {
    const child = Deno.spawnChild(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
      stdout: "null",
      stderr: "null",
    });

    child.kill("SIGKILL");
    const status = await child.status;

    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, null);
    } else {
      assertEquals(status.code, 137);
      assertEquals(status.signal, "SIGKILL");
    }
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnKillFailed() {
    const child = Deno.spawnChild(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 5000)"],
      stdout: "null",
      stderr: "null",
    });

    assertThrows(() => {
      // @ts-expect-error testing runtime error of bad signal
      child.kill("foobar");
    }, TypeError);

    await child.status;
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnKillOptional() {
    const child = Deno.spawnChild(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
      stdout: "null",
      stderr: "null",
    });

    child.kill();
    const status = await child.status;

    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, null);
    } else {
      assertEquals(status.code, 143);
      assertEquals(status.signal, "SIGTERM");
    }
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnAbort() {
    const ac = new AbortController();
    const child = Deno.spawnChild(Deno.execPath(), {
      args: [
        "eval",
        "setTimeout(console.log, 1e8)",
      ],
      signal: ac.signal,
      stdout: "null",
      stderr: "null",
    });
    queueMicrotask(() => ac.abort());
    const status = await child.status;
    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, null);
    } else {
      assertEquals(status.success, false);
      assertEquals(status.code, 143);
    }
  },
);

Deno.test(
  { permissions: { read: true, run: false } },
  async function spawnPermissions() {
    await assertRejects(async () => {
      await Deno.spawn(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      });
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { read: true, run: false } },
  function spawnSyncPermissions() {
    assertThrows(() => {
      Deno.spawnSync(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      });
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnSuccess() {
    const { status } = await Deno.spawn(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
    });

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function spawnSyncSuccess() {
    const { status } = Deno.spawnSync(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
    });

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnUrl() {
    const { status, stdout } = await Deno.spawn(
      new URL(`file:///${Deno.execPath()}`),
      {
        args: ["eval", "console.log('hello world')"],
      },
    );

    assertEquals(new TextDecoder().decode(stdout), "hello world\n");

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function spawnSyncUrl() {
    const { status, stdout } = Deno.spawnSync(
      new URL(`file:///${Deno.execPath()}`),
      {
        args: ["eval", "console.log('hello world')"],
      },
    );

    assertEquals(new TextDecoder().decode(stdout), "hello world\n");

    assertEquals(status.success, true);
    assertEquals(status.code, 0);
    assertEquals(status.signal, null);
  },
);

Deno.test({ permissions: { run: true } }, function spawnNotFound() {
  assertThrows(
    () => Deno.spawn("this file hopefully doesn't exist"),
    Deno.errors.NotFound,
  );
});

Deno.test({ permissions: { run: true } }, function spawnSyncNotFound() {
  assertThrows(
    () => Deno.spawnSync("this file hopefully doesn't exist"),
    Deno.errors.NotFound,
  );
});

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnFailedWithCode() {
    const { status } = await Deno.spawn(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    });
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function spawnSyncFailedWithCode() {
    const { status } = Deno.spawnSync(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    });
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, null);
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  async function spawnFailedWithSignal() {
    const { status } = await Deno.spawn(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    });
    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, null);
    } else {
      assertEquals(status.code, 128 + 9);
      assertEquals(status.signal, "SIGKILL");
    }
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  function spawnSyncFailedWithSignal() {
    const { status } = Deno.spawnSync(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    });
    assertEquals(status.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(status.code, 1);
      assertEquals(status.signal, null);
    } else {
      assertEquals(status.code, 128 + 9);
      assertEquals(status.signal, "SIGKILL");
    }
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function spawnOutput() {
    const { stdout } = await Deno.spawn(Deno.execPath(), {
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
  function spawnSyncOutput() {
    const { stdout } = Deno.spawnSync(Deno.execPath(), {
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
  async function spawnStderrOutput() {
    const { stderr } = await Deno.spawn(Deno.execPath(), {
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
  function spawnSyncStderrOutput() {
    const { stderr } = Deno.spawnSync(Deno.execPath(), {
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
  async function spawnEnv() {
    const { stdout } = await Deno.spawn(Deno.execPath(), {
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
  function spawnEnv() {
    const { stdout } = Deno.spawnSync(Deno.execPath(), {
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
  async function spawnClearEnv() {
    const { stdout } = await Deno.spawn(Deno.execPath(), {
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
  function spawnSyncClearEnv() {
    const { stdout } = Deno.spawnSync(Deno.execPath(), {
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
  async function spawnUid() {
    const { stdout } = await Deno.spawn("id", {
      args: ["-u"],
    });

    const currentUid = new TextDecoder().decode(stdout);

    if (currentUid !== "0") {
      await assertRejects(async () => {
        await Deno.spawn("echo", {
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
  function spawnSyncUid() {
    const { stdout } = Deno.spawnSync("id", {
      args: ["-u"],
    });

    const currentUid = new TextDecoder().decode(stdout);

    if (currentUid !== "0") {
      assertThrows(() => {
        Deno.spawnSync("echo", {
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
  async function spawnGid() {
    const { stdout } = await Deno.spawn("id", {
      args: ["-g"],
    });

    const currentGid = new TextDecoder().decode(stdout);

    if (currentGid !== "0") {
      await assertRejects(async () => {
        await Deno.spawn("echo", {
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
  function spawnSyncGid() {
    const { stdout } = Deno.spawnSync("id", {
      args: ["-g"],
    });

    const currentGid = new TextDecoder().decode(stdout);

    if (currentGid !== "0") {
      assertThrows(() => {
        Deno.spawnSync("echo", {
          args: ["fhqwhgads"],
          gid: 0,
        });
      }, Deno.errors.PermissionDenied);
    }
  },
);

Deno.test(function spawnStdinPipedFails() {
  assertThrows(
    () =>
      Deno.spawn("id", {
        stdin: "piped",
      }),
    TypeError,
    "Piped stdin is not supported for this function, use 'Deno.spawnChild()' instead",
  );
});

Deno.test(function spawnSyncStdinPipedFails() {
  assertThrows(
    () =>
      Deno.spawnSync("id", {
        stdin: "piped",
      }),
    TypeError,
    "Piped stdin is not supported for this function, use 'Deno.spawnChild()' instead",
  );
});
