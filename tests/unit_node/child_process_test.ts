// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import CP from "node:child_process";
import { Buffer } from "node:buffer";
import {
  assert,
  assertEquals,
  assertExists,
  assertNotStrictEquals,
  assertStrictEquals,
  assertStringIncludes,
  assertThrows,
} from "@std/assert";
import * as path from "@std/path";
import { clearTimeout, setTimeout } from "node:timers";

const { spawn, spawnSync, execFile, execFileSync, ChildProcess } = CP;

function withTimeout<T>(
  timeoutInMS = 10_000,
): ReturnType<typeof Promise.withResolvers<T>> {
  const deferred = Promise.withResolvers<T>();
  const timer = setTimeout(() => {
    deferred.reject("Timeout");
  }, timeoutInMS);
  deferred.promise.then(() => {
    clearTimeout(timer);
  });
  return deferred;
}

// TODO(uki00a): Once Node.js's `parallel/test-child-process-spawn-error.js` works, this test case should be removed.
Deno.test("[node/child_process spawn] The 'error' event is emitted when no binary is found", async () => {
  const deferred = withTimeout<void>();
  const childProcess = spawn("no-such-cmd");
  childProcess.on("error", (_err: Error) => {
    // TODO(@bartlomieju) Assert an error message.
    deferred.resolve();
  });
  await deferred.promise;
});

Deno.test("[node/child_process spawn] The 'exit' event is emitted with an exit code after the child process ends", async () => {
  const deferred = withTimeout<void>();
  const childProcess = spawn(Deno.execPath(), ["--help"], {
    env: { NO_COLOR: "true" },
  });
  try {
    let exitCode = null;
    childProcess.on("exit", (code: number) => {
      deferred.resolve();
      exitCode = code;
    });
    await deferred.promise;
    assertStrictEquals(exitCode, 0);
    assertStrictEquals(childProcess.exitCode, exitCode);
  } finally {
    childProcess.kill();
    childProcess.stdout?.destroy();
    childProcess.stderr?.destroy();
  }
});

Deno.test("[node/child_process disconnect] the method exists", async () => {
  const deferred = withTimeout<void>();
  const childProcess = spawn(Deno.execPath(), ["--help"], {
    env: { NO_COLOR: "true" },
    stdio: ["pipe", "pipe", "pipe", "ipc"],
  });
  try {
    childProcess.disconnect();
    childProcess.on("exit", () => {
      deferred.resolve();
    });
    await deferred.promise;
  } finally {
    childProcess.kill();
    childProcess.stdout?.destroy();
    childProcess.stderr?.destroy();
  }
});

Deno.test({
  name: "[node/child_process spawn] Verify that stdin and stdout work",
  fn: async () => {
    const deferred = withTimeout<void>();
    const childProcess = spawn(Deno.execPath(), ["fmt", "-"], {
      env: { NO_COLOR: "true" },
      stdio: ["pipe", "pipe"],
    });
    try {
      assert(childProcess.stdin, "stdin should be defined");
      assert(childProcess.stdout, "stdout should be defined");
      let data = "";
      childProcess.stdout.on("data", (chunk) => {
        data += chunk;
      });
      childProcess.stdin.write("  console.log('hello')", "utf-8");
      childProcess.stdin.end();
      childProcess.on("close", () => {
        deferred.resolve();
      });
      await deferred.promise;
      assertStrictEquals(data, `console.log("hello");\n`);
    } finally {
      childProcess.kill();
    }
  },
});

Deno.test({
  name: "[node/child_process spawn] stdin and stdout with binary data",
  fn: async () => {
    const deferred = withTimeout<void>();
    const p = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "./testdata/binary_stdio.js",
    );
    const childProcess = spawn(Deno.execPath(), ["run", p], {
      env: { NO_COLOR: "true" },
      stdio: ["pipe", "pipe"],
    });
    try {
      assert(childProcess.stdin, "stdin should be defined");
      assert(childProcess.stdout, "stdout should be defined");
      let data: Buffer;
      childProcess.stdout.on("data", (chunk) => {
        data = chunk;
      });
      const buffer = new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
      childProcess.stdin.write(buffer);
      childProcess.stdin.end();
      childProcess.on("close", () => {
        deferred.resolve();
      });
      await deferred.promise;
      assertEquals(new Uint8Array(data!), buffer);
    } finally {
      childProcess.kill();
    }
  },
});

async function spawnAndGetEnvValue(
  inputValue: string | number | boolean,
): Promise<string> {
  const deferred = withTimeout<string>();
  const env = spawn(
    `"${Deno.execPath()}" eval -p "Deno.env.toObject().BAZ"`,
    {
      env: { BAZ: String(inputValue), NO_COLOR: "true" },
      shell: true,
    },
  );
  try {
    let envOutput = "";

    assert(env.stdout);
    env.on("error", (err: Error) => deferred.reject(err));
    env.stdout.on("data", (data) => {
      envOutput += data;
    });
    env.on("close", () => {
      deferred.resolve(envOutput.trim());
    });
    return await deferred.promise;
  } finally {
    env.kill();
  }
}

Deno.test({
  ignore: Deno.build.os === "windows",
  name:
    "[node/child_process spawn] Verify that environment values can be numbers",
  async fn() {
    const envOutputValue = await spawnAndGetEnvValue(42);
    assertStrictEquals(envOutputValue, "42");
  },
});

Deno.test({
  ignore: Deno.build.os === "windows",
  name:
    "[node/child_process spawn] Verify that environment values can be booleans",
  async fn() {
    const envOutputValue = await spawnAndGetEnvValue(false);
    assertStrictEquals(envOutputValue, "false");
  },
});

/* Start of ported part */
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// Ported from Node 15.5.1

// TODO(uki00a): Remove this case once Node's `parallel/test-child-process-spawn-event.js` works.
Deno.test("[child_process spawn] 'spawn' event", async () => {
  const timeout = withTimeout<void>();
  const subprocess = spawn(Deno.execPath(), ["eval", "console.log('ok')"]);

  let didSpawn = false;
  subprocess.on("spawn", function () {
    didSpawn = true;
  });

  function mustNotBeCalled() {
    timeout.reject(new Error("function should not have been called"));
  }

  const promises = [] as Promise<void>[];
  function mustBeCalledAfterSpawn() {
    const deferred = Promise.withResolvers<void>();
    promises.push(deferred.promise);
    return () => {
      if (didSpawn) {
        deferred.resolve();
      } else {
        deferred.reject(
          new Error("function should be called after the 'spawn' event"),
        );
      }
    };
  }

  subprocess.on("error", mustNotBeCalled);
  subprocess.stdout!.on("data", mustBeCalledAfterSpawn());
  subprocess.stdout!.on("end", mustBeCalledAfterSpawn());
  subprocess.stdout!.on("close", mustBeCalledAfterSpawn());
  subprocess.stderr!.on("data", mustNotBeCalled);
  subprocess.stderr!.on("end", mustBeCalledAfterSpawn());
  subprocess.stderr!.on("close", mustBeCalledAfterSpawn());
  subprocess.on("exit", mustBeCalledAfterSpawn());
  subprocess.on("close", mustBeCalledAfterSpawn());

  try {
    await Promise.race([Promise.all(promises), timeout.promise]);
    timeout.resolve();
  } finally {
    subprocess.kill();
  }
});

// TODO(uki00a): Remove this case once Node's `parallel/test-child-process-spawn-shell.js` works.
Deno.test("[child_process spawn] Verify that a shell is executed", async () => {
  const deferred = withTimeout<void>();
  const doesNotExist = spawn("does-not-exist", { shell: true });
  try {
    assertNotStrictEquals(doesNotExist.spawnfile, "does-not-exist");
    doesNotExist.on("error", () => {
      deferred.reject("The 'error' event must not be emitted.");
    });
    doesNotExist.on("exit", (code: number, signal: null) => {
      assertStrictEquals(signal, null);

      if (Deno.build.os === "windows") {
        assertStrictEquals(code, 1); // Exit code of cmd.exe
      } else {
        assertStrictEquals(code, 127); // Exit code of /bin/sh });
      }

      deferred.resolve();
    });
    await deferred.promise;
  } finally {
    doesNotExist.kill();
    doesNotExist.stdout?.destroy();
    doesNotExist.stderr?.destroy();
  }
});

// TODO(uki00a): Remove this case once Node's `parallel/test-child-process-spawn-shell.js` works.
Deno.test({
  ignore: Deno.build.os === "windows",
  name: "[node/child_process spawn] Verify that passing arguments works",
  async fn() {
    const deferred = withTimeout<void>();
    const echo = spawn("echo", ["foo"], {
      shell: true,
    });
    let echoOutput = "";

    try {
      assertStrictEquals(
        echo.spawnargs[echo.spawnargs.length - 1].replace(/"/g, ""),
        "echo foo",
      );
      assert(echo.stdout);
      echo.stdout.on("data", (data) => {
        echoOutput += data;
      });
      echo.on("close", () => {
        assertStrictEquals(echoOutput.trim(), "foo");
        deferred.resolve();
      });
      await deferred.promise;
    } finally {
      echo.kill();
    }
  },
});

// TODO(uki00a): Remove this case once Node's `parallel/test-child-process-spawn-shell.js` works.
Deno.test({
  ignore: Deno.build.os === "windows",
  name: "[node/child_process spawn] Verity that shell features can be used",
  async fn() {
    const deferred = withTimeout<void>();
    const cmd = "echo bar | cat";
    const command = spawn(cmd, {
      shell: true,
    });
    try {
      let commandOutput = "";

      assert(command.stdout);
      command.stdout.on("data", (data) => {
        commandOutput += data;
      });

      command.on("close", () => {
        assertStrictEquals(commandOutput.trim(), "bar");
        deferred.resolve();
      });

      await deferred.promise;
    } finally {
      command.kill();
    }
  },
});

// TODO(uki00a): Remove this case once Node's `parallel/test-child-process-spawn-shell.js` works.
Deno.test({
  ignore: Deno.build.os === "windows",
  name:
    "[node/child_process spawn] Verity that environment is properly inherited",
  async fn() {
    const deferred = withTimeout<void>();
    const env = spawn(
      `"${Deno.execPath()}" eval -p "Deno.env.toObject().BAZ"`,
      {
        env: { BAZ: "buzz", NO_COLOR: "true" },
        shell: true,
      },
    );
    try {
      let envOutput = "";

      assert(env.stdout);
      env.on("error", (err: Error) => deferred.reject(err));
      env.stdout.on("data", (data) => {
        envOutput += data;
      });
      env.on("close", () => {
        assertStrictEquals(envOutput.trim(), "buzz");
        deferred.resolve();
      });
      await deferred.promise;
    } finally {
      env.kill();
    }
  },
});
/* End of ported part */

Deno.test({
  name: "[node/child_process execFile] Get stdout as a string",
  async fn() {
    let child: unknown;
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "./testdata/exec_file_text_output.js",
    );
    const promise = new Promise<string | null>((resolve, reject) => {
      child = execFile(Deno.execPath(), ["run", script], (err, stdout) => {
        if (err) reject(err);
        else if (stdout) resolve(stdout as string);
        else resolve(null);
      });
    });
    try {
      const stdout = await promise;
      assertEquals(stdout, "Hello World!\n");
    } finally {
      if (child instanceof ChildProcess) {
        child.kill();
      }
    }
  },
});

Deno.test({
  name: "[node/child_process execFile] Get stdout as a buffer",
  async fn() {
    let child: unknown;
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "./testdata/exec_file_text_output.js",
    );
    const promise = new Promise<Buffer | null>((resolve, reject) => {
      child = execFile(
        Deno.execPath(),
        ["run", script],
        { encoding: "buffer" },
        (err, stdout) => {
          if (err) reject(err);
          else if (stdout) resolve(stdout as Buffer);
          else resolve(null);
        },
      );
    });
    try {
      const stdout = await promise;
      assert(Buffer.isBuffer(stdout));
      assertEquals(stdout.toString("utf8"), "Hello World!\n");
    } finally {
      if (child instanceof ChildProcess) {
        child.kill();
      }
    }
  },
});

Deno.test({
  name: "[node/child_process execFile] Get stderr",
  async fn() {
    let child: unknown;
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "./testdata/exec_file_text_error.js",
    );
    const promise = new Promise<
      { err: Error | null; stderr?: string | Buffer }
    >((resolve) => {
      child = execFile(Deno.execPath(), ["run", script], (err, _, stderr) => {
        resolve({ err, stderr });
      });
    });
    try {
      const { err, stderr } = await promise;
      if (child instanceof ChildProcess) {
        assertEquals(child.exitCode, 1);
        assertEquals(stderr, "yikes!\n");
      } else {
        throw err;
      }
    } finally {
      if (child instanceof ChildProcess) {
        child.kill();
      }
    }
  },
});

Deno.test({
  name: "[node/child_process execFile] Exceed given maxBuffer limit",
  async fn() {
    let child: unknown;
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "./testdata/exec_file_text_error.js",
    );
    const promise = new Promise<
      { err: Error | null; stderr?: string | Buffer }
    >((resolve) => {
      child = execFile(Deno.execPath(), ["run", script], {
        encoding: "buffer",
        maxBuffer: 3,
      }, (err, _, stderr) => {
        resolve({ err, stderr });
      });
    });
    try {
      const { err, stderr } = await promise;
      if (child instanceof ChildProcess) {
        assert(err);
        assertEquals(
          // deno-lint-ignore no-explicit-any
          (err as any).code,
          "ERR_CHILD_PROCESS_STDIO_MAXBUFFER",
        );
        assertEquals(err.message, "stderr maxBuffer length exceeded");
        assertEquals((stderr as Buffer).toString("utf8"), "yik");
      } else {
        throw err;
      }
    } finally {
      if (child instanceof ChildProcess) {
        child.kill();
      }
    }
  },
});

Deno.test({
  name: "[node/child_process] ChildProcess.kill()",
  async fn() {
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "./testdata/infinite_loop.js",
    );
    const childProcess = spawn(Deno.execPath(), ["run", script]);
    const p = withTimeout<void>();
    const pStdout = withTimeout<void>();
    const pStderr = withTimeout<void>();
    childProcess.on("exit", () => p.resolve());
    childProcess.stdout.on("close", () => pStdout.resolve());
    childProcess.stderr.on("close", () => pStderr.resolve());
    childProcess.kill("SIGKILL");
    await p.promise;
    await pStdout.promise;
    await pStderr.promise;
    assert(childProcess.killed);
    assertEquals(childProcess.signalCode, "SIGKILL");
    assertExists(childProcess.exitCode);
  },
});

Deno.test({
  ignore: true,
  name: "[node/child_process] ChildProcess.unref()",
  async fn() {
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "testdata",
      "child_process_unref.js",
    );
    const childProcess = spawn(Deno.execPath(), [
      "run",
      "-A",
      script,
    ]);
    const deferred = Promise.withResolvers<void>();
    childProcess.on("exit", () => deferred.resolve());
    await deferred.promise;
  },
});

Deno.test({
  ignore: true,
  name: "[node/child_process] child_process.fork",
  async fn() {
    const testdataDir = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "testdata",
    );
    const script = path.join(
      testdataDir,
      "node_modules",
      "foo",
      "index.js",
    );
    const p = Promise.withResolvers<void>();
    const cp = CP.fork(script, [], { cwd: testdataDir, stdio: "pipe" });
    let output = "";
    cp.on("close", () => p.resolve());
    cp.stdout?.on("data", (data) => {
      output += data;
    });
    await p.promise;
    assertEquals(output, "foo\ntrue\ntrue\ntrue\n");
  },
});

Deno.test("[node/child_process execFileSync] 'inherit' stdout and stderr", () => {
  execFileSync(Deno.execPath(), ["--help"], { stdio: "inherit" });
});

Deno.test(
  "[node/child_process spawn] supports windowsVerbatimArguments option",
  { ignore: Deno.build.os !== "windows" },
  async () => {
    const cmdFinished = Promise.withResolvers<void>();
    let output = "";
    const cp = spawn("cmd", ["/d", "/s", "/c", '"deno ^"--version^""'], {
      stdio: "pipe",
      windowsVerbatimArguments: true,
    });
    cp.on("close", () => cmdFinished.resolve());
    cp.stdout?.on("data", (data) => {
      output += data;
    });
    await cmdFinished.promise;
    assertStringIncludes(output, "deno");
    assertStringIncludes(output, "v8");
    assertStringIncludes(output, "typescript");
  },
);

Deno.test(
  "[node/child_process spawn] supports stdio array option",
  async () => {
    const cmdFinished = Promise.withResolvers<void>();
    let output = "";
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "testdata",
      "child_process_stdio.js",
    );
    const cp = spawn(Deno.execPath(), ["run", "-A", script]);
    cp.stdout?.on("data", (data) => {
      output += data;
    });
    cp.on("close", () => cmdFinished.resolve());
    await cmdFinished.promise;

    assertStringIncludes(output, "foo");
    assertStringIncludes(output, "close");
  },
);

Deno.test(
  "[node/child_process spawn] supports stdio [0, 1, 2] option",
  async () => {
    const cmdFinished = Promise.withResolvers<void>();
    let output = "";
    const script = path.join(
      path.dirname(path.fromFileUrl(import.meta.url)),
      "testdata",
      "child_process_stdio_012.js",
    );
    const cp = spawn(Deno.execPath(), ["run", "-A", script]);
    cp.stdout?.on("data", (data) => {
      output += data;
    });
    cp.on("close", () => cmdFinished.resolve());
    await cmdFinished.promise;

    assertStringIncludes(output, "foo");
    assertStringIncludes(output, "close");
  },
);

Deno.test({
  name: "[node/child_process spawn] supports SIGIOT signal",
  ignore: Deno.build.os === "windows",
  async fn() {
    // Note: attempting to kill Deno with SIGABRT causes the process to zombify on certain OSX builds
    // eg: 22.5.0 Darwin Kernel Version 22.5.0: Mon Apr 24 20:53:19 PDT 2023; root:xnu-8796.121.2~5/RELEASE_ARM64_T6020 arm64
    // M2 Pro running Ventura 13.4

    // Spawn an infinite cat
    const cp = spawn("cat", ["-"]);
    const p = withTimeout<void>();
    const pStdout = withTimeout<void>();
    const pStderr = withTimeout<void>();
    cp.on("exit", () => p.resolve());
    cp.stdout.on("close", () => pStdout.resolve());
    cp.stderr.on("close", () => pStderr.resolve());
    cp.kill("SIGIOT");
    await p.promise;
    await pStdout.promise;
    await pStderr.promise;
    assert(cp.killed);
    assertEquals(cp.signalCode, "SIGIOT");
  },
});

// Regression test for https://github.com/denoland/deno/issues/20373
Deno.test(async function undefinedValueInEnvVar() {
  const deferred = withTimeout<string>();
  const env = spawn(
    `"${Deno.execPath()}" eval -p "Deno.env.toObject().BAZ"`,
    {
      env: {
        BAZ: "BAZ",
        NO_COLOR: "true",
        UNDEFINED_ENV: undefined,
        // deno-lint-ignore no-explicit-any
        NULL_ENV: null as any,
      },
      shell: true,
    },
  );
  try {
    let envOutput = "";

    assert(env.stdout);
    env.on("error", (err: Error) => deferred.reject(err));
    env.stdout.on("data", (data) => {
      envOutput += data;
    });
    env.on("close", () => {
      deferred.resolve(envOutput.trim());
    });
    await deferred.promise;
  } finally {
    env.kill();
  }
  const value = await deferred.promise;
  assertEquals(value, "BAZ");
});

// Regression test for https://github.com/denoland/deno/issues/20373
Deno.test(function spawnSyncUndefinedValueInEnvVar() {
  const ret = spawnSync(
    `"${Deno.execPath()}" eval -p "Deno.env.toObject().BAZ"`,
    {
      env: {
        BAZ: "BAZ",
        NO_COLOR: "true",
        UNDEFINED_ENV: undefined,
        // deno-lint-ignore no-explicit-any
        NULL_ENV: null as any,
      },
      shell: true,
    },
  );

  assertEquals(ret.status, 0);
  assertEquals(ret.stdout.toString("utf-8").trim(), "BAZ");
});

Deno.test(function spawnSyncStdioUndefined() {
  const ret = spawnSync(
    `"${Deno.execPath()}" eval "console.log('hello');console.error('world')"`,
    {
      stdio: [undefined, undefined, undefined],
      shell: true,
    },
  );

  assertEquals(ret.status, 0);
  assertEquals(ret.stdout.toString("utf-8").trim(), "hello");
  assertEquals(ret.stderr.toString("utf-8").trim(), "world");
});

Deno.test(function spawnSyncExitNonZero() {
  const ret = spawnSync(
    `"${Deno.execPath()}" eval "Deno.exit(22)"`,
    { shell: true },
  );

  assertEquals(ret.status, 22);
});

// https://github.com/denoland/deno/issues/21630
Deno.test(async function forkIpcKillDoesNotHang() {
  const testdataDir = path.join(
    path.dirname(path.fromFileUrl(import.meta.url)),
    "testdata",
  );
  const script = path.join(
    testdataDir,
    "node_modules",
    "foo",
    "index.js",
  );
  const p = Promise.withResolvers<void>();
  const cp = CP.fork(script, [], {
    cwd: testdataDir,
    stdio: ["inherit", "inherit", "inherit", "ipc"],
  });
  cp.on("close", () => p.resolve());
  cp.kill();

  await p.promise;
});

Deno.test(async function stripForkEnableSourceMaps() {
  const testdataDir = path.join(
    path.dirname(path.fromFileUrl(import.meta.url)),
    "testdata",
  );
  const script = path.join(
    testdataDir,
    "node_modules",
    "foo",
    "check_argv.js",
  );
  const p = Promise.withResolvers<void>();
  const cp = CP.fork(script, [], {
    cwd: testdataDir,
    stdio: "pipe",
    execArgv: ["--enable-source-maps"],
  });
  let output = "";
  cp.on("close", () => p.resolve());
  cp.stdout?.on("data", (data) => {
    output += data;
    cp.kill();
  });
  await p.promise;
  assertEquals(output, "2\n");
});

Deno.test(async function execFileWithUndefinedTimeout() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  CP.execFile(
    "git",
    ["--version"],
    { timeout: undefined, encoding: "utf8" },
    (err) => {
      if (err) {
        reject(err);
        return;
      }
      resolve();
    },
  );
  await promise;
});

Deno.test(async function spawnCommandNotFoundErrno() {
  const { promise, resolve } = Promise.withResolvers<void>();
  const cp = CP.spawn("no-such-command");
  cp.on("error", (err) => {
    const errno = Deno.build.os === "windows" ? -4058 : -2;
    // @ts-ignore: errno missing from typings
    assertEquals(err.errno, errno);
    resolve();
  });
  await promise;
});

// https://github.com/denoland/deno/issues/23045
Deno.test(function spawnCommandNullStdioArray() {
  const ret = spawnSync(
    `"${Deno.execPath()}" eval "console.log('hello');console.error('world')"`,
    {
      stdio: [null, null, null],
      shell: true,
    },
  );

  assertEquals(ret.status, 0);
});

Deno.test(
  function stdinInherit() {
    const script = `
      function timeoutPromise(promise, timeout) {
        return new Promise((resolve, reject) => {
          const timeoutId = setTimeout(() => {
            Deno.exit(69);
          }, timeout);
          promise.then((value) => {
            clearTimeout(timeoutId);
            resolve(value);
          }, (reason) => {
            clearTimeout(timeoutId);
            reject(reason);
          });
        });
      }

      await timeoutPromise(Deno.stdin.read(new Uint8Array(1)), 100)
    `;

    const output = spawnSync(Deno.execPath(), ["eval", script], {
      stdio: "inherit",
    });

    // We want to timeout to occur because the stdin isn't 'null'
    assertEquals(output.status, 69);
    assertEquals(output.stdout, null);
    assertEquals(output.stderr, null);
  },
);

Deno.test(
  async function ipcSerialization() {
    const timeout = withTimeout<void>();
    const script = `
      if (typeof process.send !== "function") {
        console.error("process.send is not a function");
        process.exit(1);
      }

      class BigIntWrapper {
        constructor(value) {
          this.value = value;
        }
        toJSON() {
          return this.value.toString();
        }
      }

      const makeSab = (arr) => {
        const sab = new SharedArrayBuffer(arr.length);
        const buf = new Uint8Array(sab);
        for (let i = 0; i < arr.length; i++) {
          buf[i] = arr[i];
        }
        return buf;
      };


      const inputs = [
        "foo",
        {
          foo: "bar",
        },
        42,
        true,
        null,
        new Uint8Array([1, 2, 3]),
        {
          foo: new Uint8Array([1, 2, 3]),
          bar: makeSab([4, 5, 6]),
        },
        [1, { foo: 2 }, [3, 4]],
        new BigIntWrapper(42n),
      ];
      for (const input of inputs) {
        process.send(input);
      }
    `;
    const file = await Deno.makeTempFile();
    await Deno.writeTextFile(file, script);
    const child = CP.fork(file, [], {
      stdio: ["inherit", "inherit", "inherit", "ipc"],
    });
    const expect = [
      "foo",
      {
        foo: "bar",
      },
      42,
      true,
      null,
      [1, 2, 3],
      {
        foo: [1, 2, 3],
        bar: [4, 5, 6],
      },
      [1, { foo: 2 }, [3, 4]],
      "42",
    ];
    let i = 0;

    child.on("message", (message) => {
      assertEquals(message, expect[i]);
      i++;
    });
    child.on("close", () => timeout.resolve());
    await timeout.promise;
    assertEquals(i, expect.length);
  },
);

Deno.test(async function childProcessExitsGracefully() {
  const testdataDir = path.join(
    path.dirname(path.fromFileUrl(import.meta.url)),
    "testdata",
  );
  const script = path.join(
    testdataDir,
    "node_modules",
    "foo",
    "index.js",
  );
  const p = Promise.withResolvers<void>();
  const cp = CP.fork(script, [], {
    cwd: testdataDir,
    stdio: ["inherit", "inherit", "inherit", "ipc"],
  });
  cp.on("close", () => p.resolve());

  await p.promise;
});

Deno.test(async function killMultipleTimesNoError() {
  const loop = `
    while (true) {
      await new Promise((resolve) => setTimeout(resolve, 10000));
    }
  `;

  const timeout = withTimeout<void>();
  const file = await Deno.makeTempFile();
  await Deno.writeTextFile(file, loop);
  const child = CP.fork(file, [], {
    stdio: ["inherit", "inherit", "inherit", "ipc"],
  });
  child.on("close", () => {
    timeout.resolve();
  });
  child.kill();
  child.kill();

  // explicitly calling disconnect after kill should throw
  assertThrows(() => child.disconnect());

  await timeout.promise;
});

// Make sure that you receive messages sent before a "message" event listener is set up
Deno.test(async function bufferMessagesIfNoListener() {
  const code = `
    process.on("message", (_) => {
      process.channel.unref();
    });
    process.send("hello");
    process.send("world");
    console.error("sent messages");
  `;
  const file = await Deno.makeTempFile();
  await Deno.writeTextFile(file, code);
  const timeout = withTimeout<void>();
  const child = CP.fork(file, [], {
    stdio: ["inherit", "inherit", "pipe", "ipc"],
  });

  let got = 0;
  child.on("message", (message) => {
    if (got++ === 0) {
      assertEquals(message, "hello");
    } else {
      assertEquals(message, "world");
    }
  });
  child.on("close", () => {
    timeout.resolve();
  });
  let stderr = "";
  child.stderr?.on("data", (data) => {
    stderr += data;
    if (stderr.includes("sent messages")) {
      // now that we've set up the listeners, and the child
      // has sent the messages, we can let it exit
      child.send("ready");
    }
  });
  await timeout.promise;
  assertEquals(got, 2);
});

Deno.test(async function sendAfterClosedThrows() {
  const code = ``;
  const file = await Deno.makeTempFile();
  await Deno.writeTextFile(file, code);
  const timeout = withTimeout<void>();
  const child = CP.fork(file, [], {
    stdio: ["inherit", "inherit", "inherit", "ipc"],
  });
  child.on("error", (err) => {
    assert("code" in err);
    assertEquals(err.code, "ERR_IPC_CHANNEL_CLOSED");
    timeout.resolve();
  });
  child.on("close", () => {
    child.send("ready");
  });

  await timeout.promise;
});
