// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";
const { run, DenoError, ErrorKind } = Deno;

test(function runPermissions() {
  let caughtError = false;
  try {
    Deno.run({ args: ["python", "-c", "print('hello world')"] });
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ run: true }, async function runSuccess() {
  const p = run({
    args: ["python", "-c", "print('hello world')"]
  });
  const status = await p.status();
  console.log("status", status);
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runCommandFailedWithCode() {
  let p = run({
    args: ["python", "-c", "import sys;sys.exit(41 + 1)"]
  });
  let status = await p.status();
  assertEquals(status.success, false);
  assertEquals(status.code, 42);
  assertEquals(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runCommandFailedWithSignal() {
  if (Deno.build.os === "win") {
    return; // No signals on windows.
  }
  const p = run({
    args: ["python", "-c", "import os;os.kill(os.getpid(), 9)"]
  });
  const status = await p.status();
  assertEquals(status.success, false);
  assertEquals(status.code, undefined);
  assertEquals(status.signal, 9);
  p.close();
});

testPerm({ run: true }, function runNotFound() {
  let error;
  try {
    run({ args: ["this file hopefully doesn't exist"] });
  } catch (e) {
    error = e;
  }
  assert(error !== undefined);
  assert(error instanceof DenoError);
  assertEquals(error.kind, ErrorKind.NotFound);
});

testPerm({ write: true, run: true }, async function runWithCwdIsAsync() {
  const enc = new TextEncoder();
  const cwd = Deno.makeTempDirSync({ prefix: "deno_command_test" });

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
  const p = run({
    cwd,
    args: ["python", `${pyProgramFile}.py`]
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
});

testPerm({ run: true }, async function runStdinPiped() {
  const p = run({
    args: ["python", "-c", "import sys; assert 'hello' == sys.stdin.read();"],
    stdin: "piped"
  });
  assert(!p.stdout);
  assert(!p.stderr);

  let msg = new TextEncoder().encode("hello");
  let n = await p.stdin.write(msg);
  assertEquals(n, msg.byteLength);

  p.stdin.close();

  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runStdoutPiped() {
  const p = run({
    args: ["python", "-c", "import sys; sys.stdout.write('hello')"],
    stdout: "piped"
  });
  assert(!p.stdin);
  assert(!p.stderr);

  const data = new Uint8Array(10);
  let r = await p.stdout.read(data);
  assertEquals(r.nread, 5);
  assertEquals(r.eof, false);
  const s = new TextDecoder().decode(data.subarray(0, r.nread));
  assertEquals(s, "hello");
  r = await p.stdout.read(data);
  assertEquals(r.nread, 0);
  assertEquals(r.eof, true);
  p.stdout.close();

  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runStderrPiped() {
  const p = run({
    args: ["python", "-c", "import sys; sys.stderr.write('hello')"],
    stderr: "piped"
  });
  assert(!p.stdin);
  assert(!p.stdout);

  const data = new Uint8Array(10);
  let r = await p.stderr.read(data);
  assertEquals(r.nread, 5);
  assertEquals(r.eof, false);
  const s = new TextDecoder().decode(data.subarray(0, r.nread));
  assertEquals(s, "hello");
  r = await p.stderr.read(data);
  assertEquals(r.nread, 0);
  assertEquals(r.eof, true);
  p.stderr.close();

  const status = await p.status();
  assertEquals(status.success, true);
  assertEquals(status.code, 0);
  assertEquals(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runOutput() {
  const p = run({
    args: ["python", "-c", "import sys; sys.stdout.write('hello')"],
    stdout: "piped"
  });
  const output = await p.output();
  const s = new TextDecoder().decode(output);
  assertEquals(s, "hello");
  p.close();
});

testPerm({ run: true }, async function runEnv() {
  const p = run({
    args: [
      "python",
      "-c",
      "import os, sys; sys.stdout.write(os.environ.get('FOO', '') + os.environ.get('BAR', ''))"
    ],
    env: {
      FOO: "0123",
      BAR: "4567"
    },
    stdout: "piped"
  });
  const output = await p.output();
  const s = new TextDecoder().decode(output);
  assertEquals(s, "01234567");
  p.close();
});
