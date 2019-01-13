// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import { run, DenoError, ErrorKind } from "deno";
import * as deno from "deno";

test(function runPermissions() {
  let caughtError = false;
  try {
    deno.run({ args: ["python", "-c", "print('hello world')"] });
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ run: true }, async function runSuccess() {
  const p = run({
    args: ["python", "-c", "print('hello world')"]
  });
  const status = await p.status();
  console.log("status", status);
  assertEqual(status.success, true);
  assertEqual(status.code, 0);
  assertEqual(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runCommandFailedWithCode() {
  let p = run({
    args: ["python", "-c", "import sys;sys.exit(41 + 1)"]
  });
  let status = await p.status();
  assertEqual(status.success, false);
  assertEqual(status.code, 42);
  assertEqual(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runCommandFailedWithSignal() {
  if (deno.platform.os === "win") {
    return; // No signals on windows.
  }
  const p = run({
    args: ["python", "-c", "import os;os.kill(os.getpid(), 9)"]
  });
  const status = await p.status();
  assertEqual(status.success, false);
  assertEqual(status.code, undefined);
  assertEqual(status.signal, 9);
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
  assertEqual(error.kind, ErrorKind.NotFound);
});

testPerm({ write: true, run: true }, async function runWithCwdIsAsync() {
  const enc = new TextEncoder();
  const cwd = deno.makeTempDirSync({ prefix: "deno_command_test" });

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

  deno.writeFileSync(`${cwd}/${pyProgramFile}.py`, enc.encode(pyProgram));
  const p = run({
    cwd,
    args: ["python", `${pyProgramFile}.py`]
  });

  // Write the expected exit code *after* starting python.
  // This is how we verify that `run()` is actually asynchronous.
  const code = 84;
  deno.writeFileSync(`${cwd}/${exitCodeFile}`, enc.encode(`${code}`));

  const status = await p.status();
  assertEqual(status.success, false);
  assertEqual(status.code, code);
  assertEqual(status.signal, undefined);
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
  assertEqual(n, msg.byteLength);

  p.stdin.close();

  const status = await p.status();
  assertEqual(status.success, true);
  assertEqual(status.code, 0);
  assertEqual(status.signal, undefined);
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
  assertEqual(r.nread, 5);
  assertEqual(r.eof, false);
  const s = new TextDecoder().decode(data.subarray(0, r.nread));
  assertEqual(s, "hello");
  r = await p.stdout.read(data);
  assertEqual(r.nread, 0);
  assertEqual(r.eof, true);
  p.stdout.close();

  const status = await p.status();
  assertEqual(status.success, true);
  assertEqual(status.code, 0);
  assertEqual(status.signal, undefined);
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
  assertEqual(r.nread, 5);
  assertEqual(r.eof, false);
  const s = new TextDecoder().decode(data.subarray(0, r.nread));
  assertEqual(s, "hello");
  r = await p.stderr.read(data);
  assertEqual(r.nread, 0);
  assertEqual(r.eof, true);
  p.stderr.close();

  const status = await p.status();
  assertEqual(status.success, true);
  assertEqual(status.code, 0);
  assertEqual(status.signal, undefined);
  p.close();
});

testPerm({ run: true }, async function runOutput() {
  const p = run({
    args: ["python", "-c", "import sys; sys.stdout.write('hello')"],
    stdout: "piped"
  });
  const output = await p.output();
  const s = new TextDecoder().decode(output);
  assertEqual(s, "hello");
  p.close();
});
