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
  async function commandWithCwdIsAsync() {
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

    const command = new Deno.Command(Deno.execPath(), {
      cwd,
      args: ["run", "--allow-read", programFile],
      stdout: "inherit",
      stderr: "inherit",
    });
    const child = command.spawn();

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
  async function commandStdinPiped() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
      stdin: "piped",
      stdout: "null",
      stderr: "null",
    });
    const child = command.spawn();

    assertThrows(() => child.stdout, TypeError, "stdout is not piped");
    assertThrows(() => child.stderr, TypeError, "stderr is not piped");

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
  async function commandStdoutPiped() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
      stderr: "null",
      stdout: "piped",
    });
    const child = command.spawn();

    assertThrows(() => child.stdin, TypeError, "stdin is not piped");
    assertThrows(() => child.stderr, TypeError, "stderr is not piped");

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
  async function commandStderrPiped() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('hello'))",
      ],
      stdout: "null",
      stderr: "piped",
    });
    const child = command.spawn();

    assertThrows(() => child.stdin, TypeError, "stdin is not piped");
    assertThrows(() => child.stdout, TypeError, "stdout is not piped");

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
  async function commandRedirectStdoutStderr() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const file = await Deno.open(fileName, {
      create: true,
      write: true,
    });

    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stderr.write(new TextEncoder().encode('error\\n')); Deno.stdout.write(new TextEncoder().encode('output\\n'));",
      ],
      stdout: "piped",
      stderr: "piped",
    });
    const child = command.spawn();
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
  async function commandRedirectStdin() {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const encoder = new TextEncoder();
    await Deno.writeFile(fileName, encoder.encode("hello"));
    const file = await Deno.open(fileName);

    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "if (new TextDecoder().decode(await Deno.readAll(Deno.stdin)) !== 'hello') throw new Error('Expected \\'hello\\'')",
      ],
      stdin: "piped",
      stdout: "null",
      stderr: "null",
    });
    const child = command.spawn();
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
  async function commandKillSuccess() {
    const command = new Deno.Command(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
      stdout: "null",
      stderr: "null",
    });
    const child = command.spawn();

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
  async function commandKillFailed() {
    const command = new Deno.Command(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 5000)"],
      stdout: "null",
      stderr: "null",
    });
    const child = command.spawn();

    assertThrows(() => {
      // @ts-expect-error testing runtime error of bad signal
      child.kill("foobar");
    }, TypeError);

    await child.status;
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandKillOptional() {
    const command = new Deno.Command(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
      stdout: "null",
      stderr: "null",
    });
    const child = command.spawn();

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
  async function commandAbort() {
    const ac = new AbortController();
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "setTimeout(console.log, 1e8)",
      ],
      signal: ac.signal,
      stdout: "null",
      stderr: "null",
    });
    const child = command.spawn();
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
  async function commandPermissions() {
    await assertRejects(async () => {
      await new Deno.Command(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      }).output();
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { read: true, run: false } },
  function commandSyncPermissions() {
    assertThrows(() => {
      new Deno.Command(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      }).outputSync();
    }, Deno.errors.PermissionDenied);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandSuccess() {
    const output = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
    }).output();

    assertEquals(output.success, true);
    assertEquals(output.code, 0);
    assertEquals(output.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncSuccess() {
    const output = new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log('hello world')"],
    }).outputSync();

    assertEquals(output.success, true);
    assertEquals(output.code, 0);
    assertEquals(output.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandUrl() {
    const output = await new Deno.Command(
      new URL(`file:///${Deno.execPath()}`),
      {
        args: ["eval", "console.log('hello world')"],
      },
    ).output();

    assertEquals(new TextDecoder().decode(output.stdout), "hello world\n");

    assertEquals(output.success, true);
    assertEquals(output.code, 0);
    assertEquals(output.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncUrl() {
    const output = new Deno.Command(
      new URL(`file:///${Deno.execPath()}`),
      {
        args: ["eval", "console.log('hello world')"],
      },
    ).outputSync();

    assertEquals(new TextDecoder().decode(output.stdout), "hello world\n");

    assertEquals(output.success, true);
    assertEquals(output.code, 0);
    assertEquals(output.signal, null);
  },
);

Deno.test({ permissions: { run: true } }, function commandNotFound() {
  assertThrows(
    () => new Deno.Command("this file hopefully doesn't exist").output(),
    Deno.errors.NotFound,
  );
});

Deno.test({ permissions: { run: true } }, function commandSyncNotFound() {
  assertThrows(
    () => new Deno.Command("this file hopefully doesn't exist").outputSync(),
    Deno.errors.NotFound,
  );
});

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandFailedWithCode() {
    const output = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    }).output();
    assertEquals(output.success, false);
    assertEquals(output.code, 42);
    assertEquals(output.signal, null);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncFailedWithCode() {
    const output = new Deno.Command(Deno.execPath(), {
      args: ["eval", "Deno.exit(41 + 1)"],
    }).outputSync();
    assertEquals(output.success, false);
    assertEquals(output.code, 42);
    assertEquals(output.signal, null);
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  async function commandFailedWithSignal() {
    const output = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    }).output();
    assertEquals(output.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(output.code, 1);
      assertEquals(output.signal, null);
    } else {
      assertEquals(output.code, 128 + 9);
      assertEquals(output.signal, "SIGKILL");
    }
  },
);

Deno.test(
  {
    permissions: { run: true, read: true },
  },
  function commandSyncFailedWithSignal() {
    const output = new Deno.Command(Deno.execPath(), {
      args: ["eval", "--unstable", "Deno.kill(Deno.pid, 'SIGKILL')"],
    }).outputSync();
    assertEquals(output.success, false);
    if (Deno.build.os === "windows") {
      assertEquals(output.code, 1);
      assertEquals(output.signal, null);
    } else {
      assertEquals(output.code, 128 + 9);
      assertEquals(output.signal, "SIGKILL");
    }
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandOutput() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
    }).output();

    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "hello");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncOutput() {
    const { stdout } = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stdout.write(new TextEncoder().encode('hello'))",
      ],
    }).outputSync();

    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "hello");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandStderrOutput() {
    const { stderr } = await new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('error'))",
      ],
    }).output();

    const s = new TextDecoder().decode(stderr);
    assertEquals(s, "error");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncStderrOutput() {
    const { stderr } = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "await Deno.stderr.write(new TextEncoder().encode('error'))",
      ],
    }).outputSync();

    const s = new TextDecoder().decode(stderr);
    assertEquals(s, "error");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function commandEnv() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stdout.write(new TextEncoder().encode(Deno.env.get('FOO') + Deno.env.get('BAR')))",
      ],
      env: {
        FOO: "0123",
        BAR: "4567",
      },
    }).output();
    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "01234567");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  function commandSyncEnv() {
    const { stdout } = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "Deno.stdout.write(new TextEncoder().encode(Deno.env.get('FOO') + Deno.env.get('BAR')))",
      ],
      env: {
        FOO: "0123",
        BAR: "4567",
      },
    }).outputSync();
    const s = new TextDecoder().decode(stdout);
    assertEquals(s, "01234567");
  },
);

Deno.test(
  { permissions: { run: true, read: true, env: true } },
  async function commandClearEnv() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "-p",
        "JSON.stringify(Deno.env.toObject())",
      ],
      clearEnv: true,
      env: {
        FOO: "23147",
      },
    }).output();

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
    const { stdout } = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        "-p",
        "JSON.stringify(Deno.env.toObject())",
      ],
      clearEnv: true,
      env: {
        FOO: "23147",
      },
    }).outputSync();

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
    const { stdout } = await new Deno.Command("id", {
      args: ["-u"],
    }).output();

    const currentUid = new TextDecoder().decode(stdout);

    if (currentUid !== "0") {
      await assertRejects(async () => {
        await new Deno.Command("echo", {
          args: ["fhqwhgads"],
          uid: 0,
        }).output();
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
    const { stdout } = new Deno.Command("id", {
      args: ["-u"],
    }).outputSync();

    const currentUid = new TextDecoder().decode(stdout);

    if (currentUid !== "0") {
      assertThrows(() => {
        new Deno.Command("echo", {
          args: ["fhqwhgads"],
          uid: 0,
        }).outputSync();
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
    const { stdout } = await new Deno.Command("id", {
      args: ["-g"],
    }).output();

    const currentGid = new TextDecoder().decode(stdout);

    if (currentGid !== "0") {
      await assertRejects(async () => {
        await new Deno.Command("echo", {
          args: ["fhqwhgads"],
          gid: 0,
        }).output();
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
    const { stdout } = new Deno.Command("id", {
      args: ["-g"],
    }).outputSync();

    const currentGid = new TextDecoder().decode(stdout);

    if (currentGid !== "0") {
      assertThrows(() => {
        new Deno.Command("echo", {
          args: ["fhqwhgads"],
          gid: 0,
        }).outputSync();
      }, Deno.errors.PermissionDenied);
    }
  },
);

Deno.test(function commandStdinPipedFails() {
  assertThrows(
    () =>
      new Deno.Command("id", {
        stdin: "piped",
      }).output(),
    TypeError,
    "Piped stdin is not supported for this function, use 'Deno.Command.spawn()' instead",
  );
});

Deno.test(function spawnSyncStdinPipedFails() {
  assertThrows(
    () =>
      new Deno.Command("id", {
        stdin: "piped",
      }).outputSync(),
    TypeError,
    "Piped stdin is not supported for this function, use 'Deno.Command.spawn()' instead",
  );
});

Deno.test(
  // FIXME(bartlomieju): this test is very flaky on CI, fix it
  {
    ignore: true,
    permissions: { write: true, run: true, read: true },
  },
  async function commandChildUnref() {
    const enc = new TextEncoder();
    const cwd = await Deno.makeTempDir({ prefix: "deno_command_test" });

    const programFile = "unref.ts";
    const program = `
const command = await new Deno.Command(Deno.execPath(), {
  cwd: Deno.args[0],
  stdout: "piped",
  args: ["run", "-A", "--unstable", Deno.args[1]],
});
const child = command.spawn();
const readable = child.stdout.pipeThrough(new TextDecoderStream());
const reader = readable.getReader();
// set up an interval that will end after reading a few messages from stdout,
// to verify that stdio streams are properly unrefed
let count = 0;
let interval;
interval = setInterval(async () => {
  count += 1;
  if (count > 10) {
    clearInterval(interval);
    console.log("cleared interval");
  }
  const res = await reader.read();
  if (res.done) {
    throw new Error("stream shouldn't be done");
  }
  if (res.value.trim() != "hello from interval") {
    throw new Error("invalid message received");
  }
}, 120);
console.log("spawned pid", child.pid);
child.unref();
`;

    const childProgramFile = "unref_child.ts";
    const childProgram = `
setInterval(() => {
  console.log("hello from interval");
}, 100);
`;
    Deno.writeFileSync(`${cwd}/${programFile}`, enc.encode(program));
    Deno.writeFileSync(`${cwd}/${childProgramFile}`, enc.encode(childProgram));
    // In this subprocess we are spawning another subprocess which has
    // an infite interval set. Following call would never resolve unless
    // child process gets unrefed.
    const { success, stdout, stderr } = await new Deno.Command(
      Deno.execPath(),
      {
        cwd,
        args: ["run", "-A", "--unstable", programFile, cwd, childProgramFile],
      },
    ).output();

    assert(success);
    const stdoutText = new TextDecoder().decode(stdout);
    const stderrText = new TextDecoder().decode(stderr);
    assert(stderrText.length == 0);
    const [line1, line2] = stdoutText.split("\n");
    const pidStr = line1.split(" ").at(-1);
    assert(pidStr);
    assertEquals(line2, "cleared interval");
    const pid = Number.parseInt(pidStr, 10);
    await Deno.remove(cwd, { recursive: true });
    // Child process should have been killed when parent process exits.
    assertThrows(() => {
      Deno.kill(pid, "SIGTERM");
    }, Deno.errors.NotFound);
  },
);

Deno.test(
  { ignore: Deno.build.os !== "windows" },
  async function commandWindowsRawArguments() {
    let { success, stdout } = await new Deno.Command("cmd", {
      args: ["/d", "/s", "/c", '"deno ^"--version^""'],
      windowsRawArguments: true,
    }).output();
    assert(success);
    let stdoutText = new TextDecoder().decode(stdout);
    assertStringIncludes(stdoutText, "deno");
    assertStringIncludes(stdoutText, "v8");
    assertStringIncludes(stdoutText, "typescript");

    ({ success, stdout } = new Deno.Command("cmd", {
      args: ["/d", "/s", "/c", '"deno ^"--version^""'],
      windowsRawArguments: true,
    }).outputSync());
    assert(success);
    stdoutText = new TextDecoder().decode(stdout);
    assertStringIncludes(stdoutText, "deno");
    assertStringIncludes(stdoutText, "v8");
    assertStringIncludes(stdoutText, "typescript");
  },
);

Deno.test(
  { permissions: { read: true, run: true } },
  async function commandWithPromisePrototypeThenOverride() {
    const originalThen = Promise.prototype.then;
    try {
      Promise.prototype.then = () => {
        throw new Error();
      };
      await new Deno.Command(Deno.execPath(), {
        args: ["eval", "console.log('hello world')"],
      }).output();
    } finally {
      Promise.prototype.then = originalThen;
    }
  },
);
