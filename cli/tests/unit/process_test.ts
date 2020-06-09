// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringContains,
  unitTest,
} from "./test_util.ts";
const {
  kill,
  run,
  readFile,
  open,
  makeTempDir,
  writeFile,
  writeFileSync,
} = Deno;

unitTest(function runPermissions(): void {
  let caughtError = false;
  try {
    run({ cmd: ["python", "-c", "print('hello world')"] });
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { run: true } }, async function runSuccess(): Promise<void> {
  const p = run({
    cmd: ["python", "-c", "print('hello world')"],
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

unitTest(
  { perms: { run: true } },
  async function runCommandFailedWithCode(): Promise<void> {
    const p = run({
      cmd: ["python", "-c", "import sys;sys.exit(41 + 1)"],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, 42);
    assertEquals(status.signal, undefined);
    p.close();
  }
);

unitTest(
  {
    // No signals on windows.
    ignore: Deno.build.os === "windows",
    perms: { run: true },
  },
  async function runCommandFailedWithSignal(): Promise<void> {
    const p = run({
      cmd: ["python", "-c", "import os;os.kill(os.getpid(), 9)"],
    });
    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, undefined);
    assertEquals(status.signal, 9);
    p.close();
  }
);

unitTest({ perms: { run: true } }, function runNotFound(): void {
  let error;
  try {
    run({ cmd: ["this file hopefully doesn't exist"] });
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
    const cwd = await makeTempDir({ prefix: "deno_command_test" });

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

    writeFileSync(`${cwd}/${pyProgramFile}.py`, enc.encode(pyProgram));
    const p = run({
      cwd,
      cmd: ["python", `${pyProgramFile}.py`],
    });

    // Write the expected exit code *after* starting python.
    // This is how we verify that `run()` is actually asynchronous.
    const code = 84;
    writeFileSync(`${cwd}/${exitCodeFile}`, enc.encode(`${code}`));

    const status = await p.status();
    assertEquals(status.success, false);
    assertEquals(status.code, code);
    assertEquals(status.signal, undefined);
    p.close();
  }
);

unitTest({ perms: { run: true } }, async function runStdinPiped(): Promise<
  void
> {
  const p = run({
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
  const p = run({
    cmd: ["python", "-c", "import sys; sys.stdout.write('hello')"],
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

unitTest({ perms: { run: true } }, async function runStderrPiped(): Promise<
  void
> {
  const p = run({
    cmd: ["python", "-c", "import sys; sys.stderr.write('hello')"],
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

unitTest({ perms: { run: true } }, async function runOutput(): Promise<void> {
  const p = run({
    cmd: ["python", "-c", "import sys; sys.stdout.write('hello')"],
    stdout: "piped",
  });
  const output = await p.output();
  const s = new TextDecoder().decode(output);
  assertEquals(s, "hello");
  p.close();
});

unitTest({ perms: { run: true } }, async function runStderrOutput(): Promise<
  void
> {
  const p = run({
    cmd: ["python", "-c", "import sys; sys.stderr.write('error')"],
    stderr: "piped",
  });
  const error = await p.stderrOutput();
  const s = new TextDecoder().decode(error);
  assertEquals(s, "error");
  p.close();
});

unitTest(
  { perms: { run: true, write: true, read: true } },
  async function runRedirectStdoutStderr(): Promise<void> {
    const tempDir = await makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const file = await open(fileName, {
      create: true,
      write: true,
    });

    const p = run({
      cmd: [
        "python",
        "-c",
        "import sys; sys.stderr.write('error\\n'); sys.stdout.write('output\\n');",
      ],
      stdout: file.rid,
      stderr: file.rid,
    });

    await p.status();
    p.close();
    file.close();

    const fileContents = await readFile(fileName);
    const decoder = new TextDecoder();
    const text = decoder.decode(fileContents);

    assertStringContains(text, "error");
    assertStringContains(text, "output");
  }
);

unitTest(
  { perms: { run: true, write: true, read: true } },
  async function runRedirectStdin(): Promise<void> {
    const tempDir = await makeTempDir();
    const fileName = tempDir + "/redirected_stdio.txt";
    const encoder = new TextEncoder();
    await writeFile(fileName, encoder.encode("hello"));
    const file = await open(fileName);

    const p = run({
      cmd: ["python", "-c", "import sys; assert 'hello' == sys.stdin.read();"],
      stdin: file.rid,
    });

    const status = await p.status();
    assertEquals(status.code, 0);
    p.close();
    file.close();
  }
);

unitTest({ perms: { run: true } }, async function runEnv(): Promise<void> {
  const p = run({
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
  const p = run({
    cmd: [
      "python",
      "-c",
      "from time import sleep; import sys; sleep(10000); sys.stderr.write('error')",
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

unitTest(function signalNumbers(): void {
  if (Deno.build.os === "darwin") {
    assertEquals(Deno.Signal.SIGSTOP, 17);
  } else if (Deno.build.os === "linux") {
    assertEquals(Deno.Signal.SIGSTOP, 19);
  }
});

unitTest(function killPermissions(): void {
  let caughtError = false;
  try {
    // Unlike the other test cases, we don't have permission to spawn a
    // subprocess we can safely kill. Instead we send SIGCONT to the current
    // process - assuming that Deno does not have a special handler set for it
    // and will just continue even if a signal is erroneously sent.
    kill(Deno.pid, Deno.Signal.SIGCONT);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { run: true } }, async function killSuccess(): Promise<void> {
  const p = run({
    cmd: ["python", "-c", "from time import sleep; sleep(10000)"],
  });

  assertEquals(Deno.Signal.SIGINT, 2);
  kill(p.pid, Deno.Signal.SIGINT);
  const status = await p.status();

  assertEquals(status.success, false);
  // TODO(ry) On Linux, status.code is sometimes undefined and sometimes 1.
  // The following assert is causing this test to be flaky. Investigate and
  // re-enable when it can be made deterministic.
  // assertEquals(status.code, 1);
  // assertEquals(status.signal, Deno.Signal.SIGINT);
  p.close();
});

unitTest({ perms: { run: true } }, function killFailed(): void {
  const p = run({
    cmd: ["python", "-c", "from time import sleep; sleep(10000)"],
  });
  assert(!p.stdin);
  assert(!p.stdout);

  let err;
  try {
    kill(p.pid, 12345);
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assert(err instanceof TypeError);

  p.close();
});
