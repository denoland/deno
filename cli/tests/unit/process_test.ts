// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
  unitTest,
} from "./test_util.ts";

unitTest(function runPermissions(): void {
  assertThrows(() => {
    Deno.run({ cmd: ["printf", "hello world"] });
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { run: true } }, async function runSuccess(): Promise<void> {
  const process = Deno.run({
    cmd: ["printf", "hello world"],
    stdout: "piped",
    stderr: "null",
  });
  const status = await process.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  Deno.stdout.close();
  process.close();
});

unitTest({ perms: { run: true } }, async function runUrl(): Promise<void> {
  // FIXME help needed here
  const q = Deno.run({
    cmd: ["python", "-c", "import sys; print sys.executable"],
    stdout: "piped",
  });
  await q.status();
  const pythonPath = new TextDecoder().decode(await q.output()).trim();
  q.close();

  // FIXME help needed here
  const p = Deno.run({
    cmd: [new URL(`file:///${pythonPath}`), "-c", "print('hello world')"],
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

unitTest({ perms: { run: true } }, async function runStdinRid0(): Promise<
  void
> {
  const process = Deno.run({
    cmd: ["printf", "hello world"],
    stdin: 0,
    stdout: "piped",
    stderr: "null",
  });
  const status = await process.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  Deno.stdout.close();
  process.close();
});

unitTest({ perms: { run: true } }, function runInvalidStdio(): void {
  assertThrows(() =>
    Deno.run({
      cmd: ["printf", "hello world"],
      // @ts-expect-error because Deno.run should throw on invalid stdin.
      stdin: "a",
    })
  );
  assertThrows(() =>
    Deno.run({
      cmd: ["printf", "hello world"],
      // @ts-expect-error because Deno.run should throw on invalid stdout.
      stdout: "b",
    })
  );
  assertThrows(() =>
    Deno.run({
      cmd: ["printf", "hello world"],
      // @ts-expect-error because Deno.run should throw on invalid stderr.
      stderr: "c",
    })
  );
});

unitTest(
  { perms: { run: true } },
  async function runCommandFailedWithCode(): Promise<void> {
    // FIXME help needed here
    const p = Deno.run({
      cmd: ["python", "-c", "import sys;sys.exit(41 + 1)"],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, undefined);
    p.close();
  },
);

unitTest(
  {
    // No signals on windows.
    ignore: Deno.build.os === "windows",
    perms: { run: true },
  },
  async function runCommandFailedWithSignal(): Promise<void> {
    // FIXME help needed here
    const p = Deno.run({
      cmd: ["python", "-c", "import os;os.kill(os.getpid(), 9)"],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, 128 + 9);
    assertEquals(status.signal, 9);
    p.close();
  },
);

unitTest({ perms: { run: true } }, function runNotFound(): void {
  let error;
  try {
    Deno.run({ cmd: ["this file hopefully doesn't exist"] });
  } catch (e) {
    error = e;
  }
  assert(error !== undefined);
  assert(error instanceof Deno.errors.NotFound);
});

unitTest(
  { perms: { write: true, run: true } },
  async function runWithCwdIsAsync(): Promise<void> {
    const enc = new TextEncoder();
    const cwd = await Deno.makeTempDir({ prefix: "deno_command_test" });

    const exitCodeFile = "deno_was_here";
    const pyProgramFile = "poll_exit.py";
    const pyProgram = `
from sys import exit
from time import sleep

while True:
  try:
    with open("${exitCodeFile}", "r") as f:
      line = f.readline()
    code = int(line)
    exit(code)
  except IOError:
    # Retry if we got here before deno wrote the file.
    sleep(0.01)
    pass
`;

    Deno.writeFileSync(`${cwd}/${pyProgramFile}.py`, enc.encode(pyProgram));
    // FIXME need help here, should I be ignoring windows?
    const p = Deno.run({
      cwd,
      cmd: ["python", `${pyProgramFile}.py`],
    });

    // Write the expected exit code *after* starting python.
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

unitTest({ perms: { run: true } }, async function runStdinPiped(): Promise<
  void
> {
  // FIXME help needed here
  const p = Deno.run({
    cmd: ["python", "-c", "import sys; assert 'hello' == sys.stdin.read();"],
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

unitTest({ perms: { run: true } }, async function runStdoutPiped(): Promise<
  void
> {
  const process = Deno.run({
    cmd: ["printf", "hello"],
    stdout: "piped",
  });
  assert(!process.stdin);
  assert(!process.stderr);

  const data = new Uint8Array(10);
  let r = await process.stdout.read(data);
  if (r === null) {
    throw new Error("p.stdout.read(...) should not be null");
  }
  assertEquals(r, 5);
  const s = new TextDecoder().decode(data.subarray(0, r));
  assertEquals(s, "hello");
  r = await process.stdout.read(data);
  assertEquals(r, null);
  process.stdout.close();

  const status = await process.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  process.close();
});

unitTest({ perms: { run: true } }, async function runStderrPiped(): Promise<
  void
> {
  // FIXME correct me.
  const process = Deno.run({
    cmd: ["python", "-c", "import sys; sys.stderr.write('hello')"],
    stderr: "piped",
  });
  assert(!process.stdin);
  assert(!process.stdout);

  const data = new Uint8Array(10);
  let r = await process.stderr.read(data);
  if (r === null) {
    throw new Error("p.stderr.read should not return null here");
  }
  assertEquals(r, 5);
  const s = new TextDecoder().decode(data.subarray(0, r));
  assertEquals(s, "hello");
  r = await process.stderr.read(data);
  assertEquals(r, null);
  process.stderr!.close();

  const status = await process.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  process.close();
});

unitTest({ perms: { run: true } }, async function runOutput(): Promise<void> {
  const process = Deno.run({
    cmd: ["printf", "hello"],
    stdout: "piped",
  });
  const output = await process.output();
  const s = new TextDecoder().decode(output);
  assertEquals(s, "hello");
  process.close();
});

unitTest({ perms: { run: true } }, async function runStderrOutput(): Promise<
  void
> {
  // FIXME need help here, should I be ignoring windows?
  const process = Deno.run({
    cmd: ["python", "-c", "import sys; sys.stderr.write('error')"],
    stderr: "piped",
  });
  const error = await process.stderrOutput();
  const s = new TextDecoder().decode(error);
  assertEquals(s, "error");
  process.close();
});

unitTest(
  { perms: { run: true, write: true, read: true } },
  async function runRedirectStdoutStderr(): Promise<void> {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const file = await Deno.open(fileName, {
      create: true,
      write: true,
    });

    const process = Deno.run({
      cmd: [
        "printf",
        "error\\n output\\n",
      ],
      stdout: file.rid,
      stderr: file.rid,
    });

    await process.status();
    process.close();
    file.close();

    const fileContents = await Deno.readFile(fileName);
    const decoder = new TextDecoder();
    const text = decoder.decode(fileContents);

    assertStringIncludes(text, "error");
    assertStringIncludes(text, "output");
  },
);

unitTest(
  { perms: { run: true, write: true, read: true } },
  async function runRedirectStdin(): Promise<void> {
    const tempDir = await Deno.makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const encoder = new TextEncoder();
    await Deno.writeFile(fileName, encoder.encode("hello"));
    const file = await Deno.open(fileName);

    const process = Deno.run({
      cmd: ["printf", "hello"],
      stdin: file.rid,
      stdout: "piped",
    });

    const status = await process.status();
    assertEquals(status.code, 0);
    process.close();
    file.close();

    const fileContents = await Deno.readFile(fileName);
    const decoder = new TextDecoder();
    const textFromFile = decoder.decode(fileContents);

    const output = await process.output();
    const decodedOutput = new TextDecoder().decode(output);

    assertEquals(textFromFile, decodedOutput);
  },
);

unitTest({ perms: { run: true } }, async function runEnv(): Promise<void> {
  // FIXME need help here, should I be ignoring windows?
  const p = Deno.run({
    cmd: [
      "python",
      "-c",
      "import os, sys; sys.stdout.write(os.environ.get('FOO', '') + os.environ.get('BAR', ''))",
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

unitTest({ perms: { run: true } }, async function runClose(): Promise<void> {
  // FIXME need help here, should I be ignoring windows?
  const process = Deno.run({
    cmd: [
      "python",
      "-c",
      "from time import sleep; import sys; sleep(10000); sys.stderr.write('error')",
    ],
    stderr: "piped",
  });
  assert(!process.stdin);
  assert(!process.stdout);

  process.close();

  const data = new Uint8Array(10);
  const r = await process.stderr.read(data);
  assertEquals(r, null);
  process.stderr.close();
});

unitTest(
  { perms: { run: true } },
  async function runKillAfterStatus(): Promise<void> {
    const process = Deno.run({
      cmd: ["printf", "hello"],
    });
    await process.status();

    // On Windows the underlying Rust API returns `ERROR_ACCESS_DENIED`,
    // which serves kind of as a catch all error code. More specific
    // error codes do exist, e.g. `ERROR_WAIT_NO_CHILDREN`; it's unclear
    // why they're not returned.
    const expectedErrorType = Deno.build.os === "windows"
      ? Deno.errors.PermissionDenied
      : Deno.errors.NotFound;
    assertThrows(
      () => process.kill(Deno.Signal.SIGTERM),
      expectedErrorType,
    );

    process.close();
  },
);

unitTest(function signalNumbers(): void {
  if (Deno.build.os === "darwin") {
    assertEquals(Deno.Signal.SIGSTOP, 17);
  } else if (Deno.build.os === "linux") {
    assertEquals(Deno.Signal.SIGSTOP, 19);
  }
});

unitTest(function killPermissions(): void {
  assertThrows(() => {
    // Unlike the other test cases, we don't have permission to spawn a
    // subprocess we can safely kill. Instead we send SIGCONT to the current
    // process - assuming that Deno does not have a special handler set for it
    // and will just continue even if a signal is erroneously sent.
    Deno.kill(Deno.pid, Deno.Signal.SIGCONT);
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { run: true } }, async function killSuccess(): Promise<void> {
  const p = Deno.run({
    cmd: ["sleep", "10000)"],
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

unitTest({ perms: { run: true } }, function killFailed(): void {
  const process = Deno.run({
    cmd: ["sleep", "10000"],
  });
  assert(!process.stdin);
  assert(!process.stdout);

  assertThrows(() => {
    Deno.kill(process.pid, 12345);
  }, TypeError);

  process.close();
});
