// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-undef no-console

import process, {
  arch as importedArch,
  argv,
  argv0 as importedArgv0,
  cpuUsage as importedCpuUsage,
  env,
  execArgv as importedExecArgv,
  execPath as importedExecPath,
  getegid,
  geteuid,
  getgid,
  getuid,
  pid as importedPid,
  platform as importedPlatform,
  setegid,
  seteuid,
  setgid,
  setuid,
} from "node:process";

import { Readable } from "node:stream";
import { once } from "node:events";
import {
  assert,
  assertEquals,
  assertFalse,
  assertMatch,
  assertObjectMatch,
  assertStrictEquals,
  assertThrows,
  fail,
} from "@std/assert";
import { stripAnsiCode } from "@std/fmt/colors";
import * as path from "@std/path";
import { delay } from "@std/async/delay";
import { stub } from "@std/testing/mock";
import { execSync } from "node:child_process";

const testDir = new URL(".", import.meta.url);

function getGroupNameFromSystem(gid: number): string {
  const stdout = execSync(`grep ":${gid}:" /etc/group`).toString();
  const groupInfo = stdout.trim().split(":")[0];
  return groupInfo;
}

function getUserNameFromSystem(uid: number): string {
  const stdout = execSync(`id -un ${uid}`).toString();
  const name = stdout.trim();
  return name;
}

Deno.test({
  name: "process.cwd and process.chdir success",
  fn() {
    assertEquals(process.cwd(), Deno.cwd());

    const currentDir = Deno.cwd();

    const tempDir = Deno.makeTempDirSync();
    process.chdir(tempDir);
    assertEquals(
      Deno.realPathSync(process.cwd()),
      Deno.realPathSync(tempDir),
    );

    process.chdir(currentDir);
  },
});

Deno.test({
  name: "process.chdir failure",
  fn() {
    assertThrows(
      () => {
        process.chdir("non-existent-directory-name");
      },
      Deno.errors.NotFound,
      "file",
      // On every OS Deno returns: "No such file" except for Windows, where it's:
      // "The system cannot find the file specified. (os error 2)" so "file" is
      // the only common string here.
    );
  },
});

Deno.test({
  name: "process.version",
  fn() {
    assertEquals(typeof process, "object");
    assertEquals(typeof process.version, "string");
    assertEquals(typeof process.versions, "object");
    assertEquals(typeof process.versions.node, "string");
    assertEquals(typeof process.versions.v8, "string");
    assertEquals(typeof process.versions.uv, "string");
    assertEquals(typeof process.versions.zlib, "string");
    assertEquals(typeof process.versions.brotli, "string");
    assertEquals(typeof process.versions.ares, "string");
    assertEquals(typeof process.versions.modules, "string");
    assertEquals(typeof process.versions.nghttp2, "string");
    assertEquals(typeof process.versions.napi, "string");
    assertEquals(typeof process.versions.llhttp, "string");
    assertEquals(typeof process.versions.openssl, "string");
    assertEquals(typeof process.versions.cldr, "string");
    assertEquals(typeof process.versions.icu, "string");
    assertEquals(typeof process.versions.tz, "string");
    assertEquals(typeof process.versions.unicode, "string");
    // These two are not present in `process.versions` in Node, but we
    // add them anyway
    assertEquals(typeof process.versions.deno, "string");
    assertEquals(typeof process.versions.typescript, "string");
  },
});

Deno.test({
  name: "process.platform",
  fn() {
    const expectedOs = Deno.build.os == "windows" ? "win32" : Deno.build.os;
    assertEquals(typeof process.platform, "string");
    assertEquals(process.platform, expectedOs);
    assertEquals(typeof importedPlatform, "string");
    assertEquals(importedPlatform, expectedOs);
  },
});

Deno.test({
  name: "process.mainModule",
  fn() {
    assertEquals(process.mainModule, undefined);
    // Check that it is writable
    // @ts-ignore these are deprecated now
    process.mainModule = "foo";
    // @ts-ignore these are deprecated now
    assertEquals(process.mainModule, "foo");
  },
});

Deno.test({
  name: "process.arch",
  fn() {
    function testValue(arch: string) {
      if (Deno.build.arch == "x86_64") {
        assertEquals(arch, "x64");
      } else if (Deno.build.arch == "aarch64") {
        assertEquals(arch, "arm64");
      } else {
        throw new Error("unreachable");
      }
    }

    assertEquals(typeof process.arch, "string");
    testValue(process.arch);
    assertEquals(typeof importedArch, "string");
    testValue(importedArch);
  },
});

Deno.test({
  name: "process.pid",
  fn() {
    assertEquals(typeof process.pid, "number");
    assertEquals(process.pid, Deno.pid);
    assertEquals(typeof importedPid, "number");
    assertEquals(importedPid, Deno.pid);
  },
});

Deno.test({
  name: "process.ppid",
  fn() {
    assertEquals(typeof process.ppid, "number");
    assertEquals(process.ppid, Deno.ppid);
  },
});

Deno.test({
  name: "process.on",
  async fn() {
    assertEquals(typeof process.on, "function");

    let triggered = false;
    process.on("exit", () => {
      triggered = true;
    });
    // @ts-ignore fix the type here
    process.emit("exit");
    assert(triggered);

    const cwd = path.dirname(path.fromFileUrl(import.meta.url));

    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--quiet",
        "./testdata/process_exit.ts",
      ],
      cwd,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(stripAnsiCode(decoder.decode(stdout).trim()), "1\n2");
  },
});

Deno.test({
  name: "process.on signal",
  ignore: Deno.build.os == "windows",
  async fn() {
    let wait = "";
    const testTimeout = setTimeout(
      () => fail("Test timed out waiting for " + wait),
      10_000,
    );
    try {
      const process = new Deno.Command(Deno.execPath(), {
        args: [
          "eval",
          `
          import process from "node:process";
          setInterval(() => {}, 1000);
          process.on("SIGINT", () => {
            console.log("foo");
          });
          console.log("ready");
          `,
        ],
        stdout: "piped",
        stderr: "null",
      }).spawn();
      let output = "";
      process.stdout.pipeThrough(new TextDecoderStream()).pipeTo(
        new WritableStream({
          write(chunk) {
            console.log("chunk:", chunk);
            output += chunk;
          },
        }),
      );
      wait = "ready";
      while (!output.includes("ready\n")) {
        await delay(10);
      }
      for (let i = 0; i < 3; i++) {
        output = "";
        process.kill("SIGINT");
        wait = "foo " + i;
        while (!output.includes("foo\n")) {
          await delay(10);
        }
      }
      process.kill("SIGTERM");
      await process.status;
    } finally {
      clearTimeout(testTimeout);
    }
  },
});

Deno.test(
  { permissions: { run: true, read: true } },
  async function processKill() {
    const p = new Deno.Command(Deno.execPath(), {
      args: ["eval", "setTimeout(() => {}, 10000)"],
    }).spawn();

    // kill with signal 0 should keep the process alive in linux (true means no error happened)
    // windows ignore signals
    assertEquals(process.kill(p.pid, 0), true);
    process.kill(p.pid);
    await p.status;
  },
);

Deno.test({
  name: "process.off signal",
  ignore: Deno.build.os == "windows",
  async fn() {
    const testTimeout = setTimeout(() => fail("Test timed out"), 10_000);
    try {
      const process = new Deno.Command(Deno.execPath(), {
        args: [
          "eval",
          `
          import process from "node:process";
          setInterval(() => {}, 1000);
          const listener = () => {
            process.off("SIGINT", listener);
            console.log("foo");
          };
          process.on("SIGINT", listener);
          console.log("ready");
          `,
        ],
        stdout: "piped",
        stderr: "null",
      }).spawn();
      let output = "";
      process.stdout.pipeThrough(new TextDecoderStream()).pipeTo(
        new WritableStream({
          write(chunk) {
            console.log("chunk:", chunk);
            output += chunk;
          },
        }),
      );
      while (!output.includes("ready\n")) {
        await delay(10);
      }
      output = "";
      process.kill("SIGINT");
      while (!output.includes("foo\n")) {
        await delay(10);
      }
      process.kill("SIGINT");
      await process.status;
    } finally {
      clearTimeout(testTimeout);
    }
  },
});

// Only supported on Windows (but won't throw anywhere)
Deno.test({
  name: "process.on SIGBREAK doesn't throw",
  fn() {
    const listener = () => {};
    process.on("SIGBREAK", listener);
    process.off("SIGBREAK", listener);
  },
});

// Not supported on Windows (but won't throw anywhere)
Deno.test({
  name: "process.on SIGTERM doesn't throw",
  fn() {
    const listener = () => {};
    process.on("SIGTERM", listener);
    process.off("SIGTERM", listener);
  },
});

Deno.test({
  name: "process.argv",
  fn() {
    assert(Array.isArray(argv));
    assert(Array.isArray(process.argv));
    assert(
      process.argv[0].match(/[^/\\]*deno[^/\\]*$/),
      "deno included in the file name of argv[0]",
    );
    assertEquals(
      process.argv[1],
      path.fromFileUrl(Deno.mainModule),
    );
    // argv supports array methods.
    assert(Array.isArray(process.argv.slice(2)));
    assertEquals(process.argv.indexOf(Deno.execPath()), 0);
    assertEquals(process.argv.indexOf(path.fromFileUrl(Deno.mainModule)), 1);
  },
});

Deno.test({
  name: "process.argv0",
  async fn() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        `import process from "node:process";console.log(process.argv0);`,
      ],
      stdout: "piped",
      stderr: "null",
    }).output();
    assertEquals(new TextDecoder().decode(stdout).trim(), Deno.execPath());

    assertEquals(typeof process.argv0, "string");
    assert(
      process.argv0.match(/[^/\\]*deno[^/\\]*$/),
      "deno included in the file name of argv[0]",
    );
    assertEquals(typeof importedArgv0, "string");
    assert(
      importedArgv0.match(/[^/\\]*deno[^/\\]*$/),
      "deno included in the file name of argv[0]",
    );

    // Setting should be a noop
    process.argv0 = "foobar";
    assert(
      process.argv0.match(/[^/\\]*deno[^/\\]*$/),
      "deno included in the file name of argv[0]",
    );
  },
});

Deno.test({
  name: "process.execArgv",
  fn() {
    assert(Array.isArray(process.execArgv));
    assert(process.execArgv.length == 0);
    // execArgv supports array methods.
    assert(Array.isArray(process.argv.slice(0)));
    assertEquals(process.argv.indexOf("foo"), -1);
  },
});

Deno.test({
  name: "process.env",
  fn() {
    assert(Object.prototype.hasOwnProperty.call(process, "env"));

    Deno.env.set("HELLO", "WORLD");

    assertObjectMatch(process.env, Deno.env.toObject());

    assertEquals(typeof (process.env.HELLO), "string");
    assertEquals(process.env.HELLO, "WORLD");

    assertEquals(typeof env.HELLO, "string");
    assertEquals(env.HELLO, "WORLD");

    assert(Object.getOwnPropertyNames(process.env).includes("HELLO"));
    assert(Object.keys(process.env).includes("HELLO"));

    assert(Object.prototype.hasOwnProperty.call(process.env, "HELLO"));
    assert(
      !Object.prototype.hasOwnProperty.call(
        process.env,
        "SURELY_NON_EXISTENT_VAR",
      ),
    );

    Object.defineProperty(process.env, "HELLO", {
      value: "OTHER_WORLD",
      configurable: true,
      writable: true,
      enumerable: true,
    });
    assertEquals(process.env.HELLO, "OTHER_WORLD");

    // deno-lint-ignore no-prototype-builtins
    assert(process.env.hasOwnProperty("HELLO"));
    assert("HELLO" in process.env);
    assert(Object.keys(process.env.valueOf()).includes("HELLO"));

    assertEquals(process.env.toString(), "[object Object]");
    assertEquals(process.env.toLocaleString(), "[object Object]");

    // should not error when assigning false to an env var
    process.env.HELLO = false as unknown as string;
    assertEquals(process.env.HELLO, "false");
    process.env.HELLO = "WORLD";
    assertEquals(process.env.HELLO, "WORLD");

    delete process.env.HELLO;
    assertEquals(process.env.HELLO, undefined);
  },
});

// #30701
Deno.test({
  name: "process.env handles falsy values correctly",
  fn() {
    const key = "TEST_ENV_VAR_EMPTY_STRING";
    Deno.env.set(key, "");

    assertEquals(process.env[key], "");
    assertEquals(Object.keys(process.env).includes(key), true);
    assert(key in process.env);
    assert(Object.hasOwn(process.env, key));
  },
});

Deno.test({
  name: "process.env requires scoped env permission",
  permissions: { env: ["FOO"] },
  fn() {
    Deno.env.set("FOO", "1");
    assert("FOO" in process.env);
    assertThrows(() => {
      process.env.BAR;
    }, Deno.errors.NotCapable);
    assert(Object.hasOwn(process.env, "FOO"));
  },
});

Deno.test({
  name: "process.env doesn't throw with invalid env var names",
  fn() {
    assertEquals(process.env[""], undefined);
    assertEquals(process.env["\0"], undefined);
    assertEquals(process.env["=c:"], undefined);
    assertFalse(Object.hasOwn(process.env, ""));
    assertFalse(Object.hasOwn(process.env, "\0"));
    assertFalse(Object.hasOwn(process.env, "=c:"));
    assertFalse("" in process.env);
    assertFalse("\0" in process.env);
    assertFalse("=c:" in process.env);
  },
});

Deno.test({
  "name": "process.env: checking symbol in env should not require permission",
  permissions: "none",
  fn() {
    const symbol = Symbol.for("67");
    Reflect.has(globalThis.process.env, symbol);
  },
});

Deno.test({
  // NB(Tango992): Node.js does not support using symbols as env keys,
  // thus this test should be omitted once we align with Node.js behavior.
  name: "process.env: setting and getting a symbol key",
  fn() {
    const symbol = Symbol.for("foo");
    // @ts-expect-error setting a symbol key
    process.env[symbol] = "foo";
    // @ts-expect-error getting a symbol key
    assertEquals(process.env[symbol], "foo");
    assert(Reflect.has(process.env, symbol));

    // @ts-expect-error deleting a symbol key
    delete process.env[symbol];
    assertFalse(Reflect.has(process.env, symbol));

    Object.defineProperty(process.env, symbol, {
      value: "bar",
      configurable: true,
      writable: true,
      enumerable: true,
    });
    // @ts-expect-error getting a symbol key
    assertEquals(process.env[symbol], "bar");
  },
});

Deno.test({
  name: "process.stdin",
  fn() {
    // @ts-ignore `Deno.stdin.rid` was soft-removed in Deno 2.
    assertEquals(process.stdin.fd, Deno.stdin.rid);
    const isTTY = Deno.stdin.isTerminal();
    assertEquals(process.stdin.isTTY, isTTY);

    // Allows overwriting `process.stdin.isTTY` (mirrors stdout/stderr from #26130)
    const original = process.stdin.isTTY;
    try {
      // @ts-ignore isTTY is defined as readonly in types but we allow setting it
      process.stdin.isTTY = !isTTY;
      assertEquals(process.stdin.isTTY, !isTTY);
    } finally {
      // @ts-ignore isTTY is defined as readonly in types but we allow setting it
      process.stdin.isTTY = original;
    }
  },
});

Deno.test({
  name: "process.stdin readable with a TTY",
  // TODO(PolarETech): Run this test even in non tty environment
  ignore: !Deno.stdin.isTerminal(),
  // stdin resource is present before the test starts.
  sanitizeResources: false,
  async fn() {
    const { promise, resolve } = Promise.withResolvers<void>();
    const expected = ["foo", "bar", null, "end"];
    const data: (string | null)[] = [];

    process.stdin.setEncoding("utf8");
    process.stdin.on("readable", () => {
      data.push(process.stdin.read());
    });
    process.stdin.on("end", () => {
      data.push("end");
    });

    process.stdin.push("foo");
    process.nextTick(() => {
      process.stdin.push("bar");
      process.nextTick(() => {
        process.stdin.push(null);
        resolve();
      });
    });

    await promise;
    assertEquals(process.stdin.readableHighWaterMark, 0);
    assertEquals(data, expected);
  },
});

Deno.test({
  name: "process.stdin readable with piping a file",
  async fn() {
    const expected = ["65536", "foo", "bar", "null", "end"];
    const scriptPath = "./testdata/process_stdin.ts";
    const filePath = "./testdata/process_stdin_dummy.txt";

    const shell = Deno.build.os === "windows" ? "cmd.exe" : "/bin/sh";
    const cmd = `"${Deno.execPath()}" run ${scriptPath} < ${filePath}`;
    const args = Deno.build.os === "windows" ? ["/d", "/c", cmd] : ["-c", cmd];

    const p = new Deno.Command(shell, {
      args,
      stdin: "null",
      stdout: "piped",
      stderr: "null",
      windowsRawArguments: true,
      cwd: testDir,
    });

    const { stdout } = await p.output();
    const data = new TextDecoder().decode(stdout).trim().split("\n");
    assertEquals(data, expected);
  },
});

Deno.test({
  name: "process.stdin readable with piping a stream",
  async fn() {
    const expected = [
      Deno.build.os == "windows" ? "16384" : "65536",
      "foo",
      "bar",
      "null",
      "end",
    ];
    const scriptPath = "./testdata/process_stdin.ts";

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", scriptPath],
      stdin: "piped",
      stdout: "piped",
      stderr: "null",
      cwd: testDir,
    });
    const child = command.spawn();

    const writer = await child.stdin.getWriter();
    writer.ready
      .then(() => writer.write(new TextEncoder().encode("foo\nbar")))
      .then(() => writer.releaseLock())
      .then(() => child.stdin.close());

    const { stdout } = await child.output();
    const data = new TextDecoder().decode(stdout).trim().split("\n");
    assertEquals(data, expected);
  },
});

Deno.test({
  name: "process.stdin readable with piping a socket",
  ignore: Deno.build.os === "windows",
  async fn() {
    const expected = ["65536", "foo", "bar", "null", "end"];
    const scriptPath = "./testdata/process_stdin.ts";

    const listener = Deno.listen({ hostname: "127.0.0.1", port: 9000 });
    listener.accept().then(async (conn) => {
      await conn.write(new TextEncoder().encode("foo\nbar"));
      conn.close();
      listener.close();
    });

    const shell = "/bin/bash";
    const cmd =
      `"${Deno.execPath()}" run ${scriptPath} < /dev/tcp/127.0.0.1/9000`;
    const args = ["-c", cmd];

    const p = new Deno.Command(shell, {
      args,
      stdin: "null",
      stdout: "piped",
      stderr: "null",
      cwd: testDir,
    });

    const { stdout } = await p.output();
    const data = new TextDecoder().decode(stdout).trim().split("\n");
    assertEquals(data, expected);
  },
});

Deno.test({
  name: "process.stdin readable with null",
  async fn() {
    const expected = ["65536", "null", "end"];
    const scriptPath = "./testdata/process_stdin.ts";

    const command = new Deno.Command(Deno.execPath(), {
      args: ["run", scriptPath],
      stdin: "null",
      stdout: "piped",
      stderr: "null",
      cwd: testDir,
    });

    const { stdout } = await command.output();
    const data = new TextDecoder().decode(stdout).trim().split("\n");
    assertEquals(data, expected);
  },
});

// TODO(kt3k): Enable this test case. 'readable' event handler in
// `process_stdin.ts` doesn't work now
Deno.test({
  name: "process.stdin readable with unsuitable stdin",
  ignore: true,
  // // TODO(PolarETech): Prepare a similar test that can be run on Windows
  // ignore: Deno.build.os === "windows",
  async fn() {
    const expected = ["65536", "null", "end"];
    const scriptPath = "./testdata/process_stdin.ts";
    const directoryPath = "./testdata/";

    const shell = "/bin/bash";
    const cmd = `"${Deno.execPath()}" run ${scriptPath} < ${directoryPath}`;
    const args = ["-c", cmd];

    const p = new Deno.Command(shell, {
      args,
      stdin: "null",
      stdout: "piped",
      stderr: "null",
      windowsRawArguments: true,
      cwd: testDir,
    });

    const { stdout } = await p.output();
    const data = new TextDecoder().decode(stdout).trim().split("\n");
    assertEquals(data, expected);
  },
});

Deno.test({
  name: "process.stdout",
  fn() {
    // @ts-ignore `Deno.stdout.rid` was soft-removed in Deno 2.
    assertEquals(process.stdout.fd, Deno.stdout.rid);
    const isTTY = Deno.stdout.isTerminal();
    assertEquals(process.stdout.isTTY, isTTY);
    const consoleSize = isTTY ? Deno.consoleSize() : undefined;
    assertEquals(process.stdout.columns, consoleSize?.columns);
    assertEquals(process.stdout.rows, consoleSize?.rows);
    assert([1, 4, 8, 24].includes(process.stdout.getColorDepth()));
    assertEquals(
      `${process.stdout.getWindowSize()}`,
      `${consoleSize && [consoleSize.columns, consoleSize.rows]}`,
    );

    if (isTTY) {
      assertStrictEquals(process.stdout.cursorTo(1, 2, () => {}), true);
      assertStrictEquals(process.stdout.moveCursor(3, 4, () => {}), true);
      assertStrictEquals(process.stdout.clearLine(1, () => {}), true);
      assertStrictEquals(process.stdout.clearScreenDown(() => {}), true);
    } else {
      assertStrictEquals(process.stdout.cursorTo, undefined);
      assertStrictEquals(process.stdout.moveCursor, undefined);
      assertStrictEquals(process.stdout.clearLine, undefined);
      assertStrictEquals(process.stdout.clearScreenDown, undefined);
    }

    // Allows overwriting `process.stdout.isTTY`
    // https://github.com/denoland/deno/issues/26123
    const original = process.stdout.isTTY;
    try {
      process.stdout.isTTY = !isTTY;
      assertEquals(process.stdout.isTTY, !isTTY);
    } finally {
      process.stdout.isTTY = original;
    }
  },
});

Deno.test({
  name: "process.stderr",
  fn() {
    // @ts-ignore `Deno.stderr.rid` was soft-removed in Deno 2.
    assertEquals(process.stderr.fd, Deno.stderr.rid);
    const isTTY = Deno.stderr.isTerminal();
    assertEquals(process.stderr.isTTY, isTTY);
    const consoleSize = isTTY ? Deno.consoleSize() : undefined;
    assertEquals(process.stderr.columns, consoleSize?.columns);
    assertEquals(process.stderr.rows, consoleSize?.rows);
    assertEquals(
      `${process.stderr.getWindowSize()}`,
      `${consoleSize && [consoleSize.columns, consoleSize.rows]}`,
    );

    if (isTTY) {
      assertStrictEquals(process.stderr.cursorTo(1, 2, () => {}), true);
      assertStrictEquals(process.stderr.moveCursor(3, 4, () => {}), true);
      assertStrictEquals(process.stderr.clearLine(1, () => {}), true);
      assertStrictEquals(process.stderr.clearScreenDown(() => {}), true);
    } else {
      assertStrictEquals(process.stderr.cursorTo, undefined);
      assertStrictEquals(process.stderr.moveCursor, undefined);
      assertStrictEquals(process.stderr.clearLine, undefined);
      assertStrictEquals(process.stderr.clearScreenDown, undefined);
    }
  },
});

Deno.test({
  name: "process.nextTick",
  async fn() {
    let withoutArguments = false;
    process.nextTick(() => {
      withoutArguments = true;
    });

    const expected = 12;
    let result;
    process.nextTick((x: number) => {
      result = x;
    }, 12);

    await delay(10);
    assert(withoutArguments);
    assertEquals(result, expected);
  },
});

Deno.test({
  name: "process.hrtime",
  // TODO(kt3k): Enable this test
  ignore: true,
  fn() {
    const [sec0, nano0] = process.hrtime();
    // seconds and nano seconds are positive integers.
    assert(sec0 > 0);
    assert(Number.isInteger(sec0));
    assert(nano0 > 0);
    assert(Number.isInteger(nano0));

    const [sec1, nano1] = process.hrtime();
    // the later call returns bigger value
    assert(sec1 >= sec0);
    assert(nano1 > nano0);

    const [sec2, nano2] = process.hrtime([sec1, nano1]);
    // the difference of the 2 calls is a small positive value.
    assertEquals(sec2, 0);
    assert(nano2 > 0);
  },
});

Deno.test({
  name: "process.hrtime.bigint",
  fn() {
    const time = process.hrtime.bigint();
    assertEquals(typeof time, "bigint");
    assert(time > 0n);
  },
});

Deno.test("process.on, process.off, process.removeListener doesn't throw on unimplemented events", () => {
  const events = [
    "beforeExit",
    "disconnect",
    "message",
    "multipleResolves",
    "rejectionHandled",
    "uncaughtException",
    "uncaughtExceptionMonitor",
    "unhandledRejection",
    "worker",
  ];
  const handler = () => {};
  events.forEach((ev) => {
    process.on(ev, handler);
    assertEquals(process.listenerCount(ev), 1);
    process.off(ev, handler);
    assertEquals(process.listenerCount(ev), 0);
    process.on(ev, handler);
    assertEquals(process.listenerCount(ev), 1);
    process.removeListener(ev, handler);
    assertEquals(process.listenerCount(ev), 0);
  });
});

Deno.test("process.memoryUsage()", () => {
  const mem = process.memoryUsage();
  assert(typeof mem.rss === "number");
  assert(typeof mem.heapTotal === "number");
  assert(typeof mem.heapUsed === "number");
  assert(typeof mem.external === "number");
  assert(typeof mem.arrayBuffers === "number");
  assertEquals(mem.arrayBuffers, 0);
});

Deno.test("process.memoryUsage.rss()", () => {
  const rss = process.memoryUsage.rss();
  assert(typeof rss === "number");
});

Deno.test("process.exitCode", () => {
  assertEquals(process.exitCode, undefined);
  process.exitCode = 127;
  assertEquals(process.exitCode, 127);
  assertThrows(() => {
    // deno-lint-ignore no-explicit-any
    (process.exitCode as any) = "asdf";
  });
  // deno-lint-ignore no-explicit-any
  (process.exitCode as any) = "10";
  process.exitCode = undefined; // reset
});

async function exitCodeTest(codeText: string, expectedExitCode: number) {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      codeText,
    ],
    cwd: testDir,
  });
  const { code } = await command.output();
  assertEquals(code, expectedExitCode);
}

Deno.test("process.exitCode in should change exit code", async () => {
  await exitCodeTest(
    "import process from 'node:process'; process.exitCode = 127;",
    127,
  );
  await exitCodeTest(
    "import process from 'node:process'; process.exitCode = '10';",
    10,
  );
  await exitCodeTest(
    "import process from 'node:process'; process.exitCode = '0x10';",
    16,
  );
});

Deno.test("Deno.exit should override process exit", async () => {
  await exitCodeTest(
    "import process from 'node:process'; process.exitCode = 10; Deno.exit(12);",
    12,
  );
});

Deno.test("process.config", () => {
  assert(process.config !== undefined);
  assert(process.config.target_defaults !== undefined);
  assert(process.config.variables !== undefined);
});

Deno.test("process._exiting", () => {
  // @ts-ignore fix the type here
  assert(process._exiting === false);
});

Deno.test("process.execPath", () => {
  assertEquals(process.execPath, process.argv[0]);
});

Deno.test("process.execPath is writable", () => {
  // pnpm writes to process.execPath
  // https://github.com/pnpm/pnpm/blob/67d8b65d2e8da1df3725034b8c5b1fcf3af4ad81/packages/config/src/index.ts#L175
  const originalExecPath = process.execPath;
  try {
    process.execPath = "/path/to/node";
    assertEquals(process.execPath, "/path/to/node");
  } finally {
    process.execPath = originalExecPath;
  }
});

Deno.test("process.getgid", () => {
  if (Deno.build.os === "windows") {
    assertEquals(process.getgid, undefined);
  } else {
    assertEquals(process.getgid?.(), Deno.gid());
  }
});

Deno.test("process.getuid", () => {
  if (Deno.build.os === "windows") {
    assertEquals(process.getuid, undefined);
  } else {
    assertEquals(process.getuid?.(), Deno.uid());
  }
});

Deno.test("process.geteuid", () => {
  if (Deno.build.os === "windows") {
    assertEquals(process.geteuid, undefined);
  } else {
    assert(geteuid);
    assert(typeof process.geteuid?.() === "number");
  }
});

Deno.test({
  name: "process.exit",
  async fn() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--quiet",
        "./testdata/process_exit2.ts",
      ],
      cwd: testDir,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(stripAnsiCode(decoder.decode(stdout).trim()), "exit");
  },
});

Deno.test({
  name: "process.reallyExit",
  async fn() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--quiet",
        "./testdata/process_really_exit.ts",
      ],
      cwd: testDir,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(stripAnsiCode(decoder.decode(stdout).trim()), "really exited");
  },
});

Deno.test({
  name: "process._rawDebug",
  async fn() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--quiet",
        "./testdata/process_raw_debug.ts",
      ],
      cwd: testDir,
    });
    const { stdout, stderr } = await command.output();

    assertEquals(stdout.length, 0);
    const decoder = new TextDecoder();
    assertEquals(
      stripAnsiCode(decoder.decode(stderr).trim()),
      "this should go to stderr { a: 1, b: [ 'a', 2 ] }",
    );
  },
});

Deno.test({
  name: "process.stdout isn't closed when source stream ended",
  async fn() {
    const source = Readable.from(["foo", "bar"]);

    source.pipe(process.stdout);
    await once(source, "end");

    // Wait a bit to ensure that streaming is completely finished.
    await delay(10);

    // This checks if the rid 1 is still valid.
    assert(typeof process.stdout.isTTY === "boolean");
  },
});

Deno.test({
  name: "process.title",
  fn() {
    assertEquals(process.title, "deno");
    // Verify that setting the value has no effect.
    process.title = "foo";
    assertEquals(process.title, "deno");
  },
});

Deno.test({
  name: "process.argv[1] in Worker",
  async fn() {
    const worker = new Worker(
      `data:text/javascript,import process from "node:process";console.log(process.argv[1]);`,
      { type: "module" },
    );
    await delay(10);
    worker.terminate();
  },
});

Deno.test({
  name: "process.binding('uv').errname",
  ignore: Deno.build.os === "windows",
  fn() {
    // @ts-ignore: untyped internal binding, not actually supposed to be
    // used by userland modules in Node.js
    const uv = process.binding("uv");
    assert(uv.errname);
    assert(typeof uv.errname === "function");
    assertEquals(uv.errname(-1), "EPERM");
  },
});

Deno.test({
  name: "process.binding('uv').getErrorMessage",
  ignore: Deno.build.os === "windows",
  fn() {
    // @ts-ignore: untyped internal binding, not actually supposed to be
    // used by userland modules in Node.js
    const uv = process.binding("uv");
    assert(uv.getErrorMessage);
    assert(typeof uv.getErrorMessage === "function");
    assertEquals(uv.getErrorMessage(-1), "operation not permitted");
  },
});

Deno.test({
  name: "process.binding('uv').getErrorMap",
  ignore: Deno.build.os === "windows",
  fn() {
    // @ts-ignore: untyped internal binding, not actually supposed to be
    // used by userland modules in Node.js
    const uv = process.binding("uv");
    assert(uv.getErrorMap);
    assert(typeof uv.getErrorMap === "function");
    const errorMap = uv.getErrorMap();
    assert(errorMap instanceof Map);
    // errorMap maps error code (number) to [name, message]
    assertEquals(errorMap.get(-1), ["EPERM", "operation not permitted"]);
  },
});

Deno.test({
  name: "process.binding('uv').getCodeMap",
  ignore: Deno.build.os === "windows",
  fn() {
    // @ts-ignore: untyped internal binding, not actually supposed to be
    // used by userland modules in Node.js
    const uv = process.binding("uv");
    assert(uv.getCodeMap);
    assert(typeof uv.getCodeMap === "function");
    const codeMap = uv.getCodeMap();
    assert(codeMap instanceof Map);
    // codeMap maps error name (string) to error code (number)
    assertEquals(codeMap.get("EPERM"), -1);
  },
});

Deno.test({
  name: "process.report",
  fn() {
    // The process.report is marked as possibly undefined in node 18 typings
    if (!process.report) throw "No process report";

    assert(typeof process.report.directory === "string");
    assert(typeof process.report.filename === "string");
    assert(typeof process.report.getReport === "function");
    assert(typeof process.report.reportOnFatalError === "boolean");
    assert(typeof process.report.reportOnSignal === "boolean");
    assert(typeof process.report.reportOnUncaughtException === "boolean");
    assert(typeof process.report.signal === "string");
    assert(typeof process.report.writeReport === "function");
  },
});

Deno.test({
  name: "process.report.writeReport unimplemented result",
  fn() {
    // The process.report is marked as possibly undefined in node 18 typings
    if (!process.report) throw "No process report";

    assertEquals(process.report.writeReport(), "");
  },
});

Deno.test({
  name: "process.report.getReport result",
  fn() {
    // The process.report is marked as possibly undefined in node 18 typings
    if (!process.report) throw "No process report";

    // deno-lint-ignore no-explicit-any
    const result = process.report.getReport() as any;

    // test and remove dynamic parts
    assert(typeof result.header.filename === "string");
    delete result.header.filename;
    assert(typeof result.header.dumpEventTime === "object");
    delete result.header.dumpEventTime;
    assert(typeof result.header.dumpEventTimeStamp === "number");
    delete result.header.dumpEventTimeStamp;
    assert(typeof result.header.processId === "number");
    delete result.header.processId;
    assert(typeof result.header.cwd === "string");
    delete result.header.cwd;
    assert(typeof result.header.nodejsVersion === "string");
    assert(result.header.nodejsVersion.startsWith("v"));
    delete result.header.nodejsVersion;
    assert(typeof result.header.arch === "string");
    delete result.header.arch;
    assert(typeof result.header.platform === "string");
    delete result.header.platform;
    assert(typeof result.header.componentVersions === "object");
    delete result.header.componentVersions;
    assert(typeof result.header.osName === "string");
    delete result.header.osName;
    assert(typeof result.header.osMachine === "string");
    delete result.header.osMachine;
    assert(Array.isArray(result.header.cpus));
    delete result.header.cpus;
    assert(typeof result.header.networkInterfaces === "object");
    delete result.header.networkInterfaces;
    assert(typeof result.header.host === "string");
    delete result.header.host;

    // test hardcoded part
    assertEquals(result, {
      header: {
        reportVersion: 3,
        event: "JavaScript API",
        trigger: "GetReport",
        threadId: 0,
        commandLine: ["node"],
        glibcVersionRuntime: "2.38",
        glibcVersionCompiler: "2.38",
        wordSize: 64,
        release: {
          name: "node",
          headersUrl:
            "https://nodejs.org/download/release/v21.2.0/node-v21.2.0-headers.tar.gz",
          sourceUrl:
            "https://nodejs.org/download/release/v21.2.0/node-v21.2.0.tar.gz",
        },
        osRelease: undefined,
        osVersion: undefined,
      },
      javascriptStack: undefined,
      javascriptHeap: undefined,
      nativeStack: undefined,
      resourceUsage: undefined,
      uvthreadResourceUsage: undefined,
      libuv: undefined,
      workers: [],
      environmentVariables: undefined,
      userLimits: undefined,
      sharedObjects: undefined,
    });
  },
});

Deno.test({
  name: "process.setSourceMapsEnabled",
  fn() {
    // @ts-ignore: setSourceMapsEnabled is not available in the types yet.
    process.setSourceMapsEnabled(false); // noop
    // @ts-ignore: setSourceMapsEnabled is not available in the types yet.
    process.setSourceMapsEnabled(true); // noop
  },
});

Deno.test({
  name: "process.sourceMapsEnabled",
  fn() {
    // @ts-ignore: not available in the types yet.
    assertEquals(process.sourceMapsEnabled, true);
  },
});

// Regression test for https://github.com/denoland/deno/issues/23761
Deno.test({
  name: "process.uptime without this",
  fn() {
    const v = (0, process.uptime)();
    assert(v >= 0);
  },
});

// Test for https://github.com/denoland/deno/issues/23863
Deno.test({
  name: "instantiate process constructor without 'new' keyword",
  fn() {
    // This would throw
    process.constructor.call({});
  },
});

// Test for https://github.com/denoland/deno/issues/22892
Deno.test("process.listeners - include SIG* events", () => {
  const listener = () => console.log("SIGINT");
  process.on("SIGINT", listener);
  assertEquals(process.listeners("SIGINT").length, 1);

  const listener2 = () => console.log("SIGINT");
  process.prependListener("SIGINT", listener2);
  assertEquals(process.listeners("SIGINT").length, 2);

  process.off("SIGINT", listener);
  assertEquals(process.listeners("SIGINT").length, 1);
  process.off("SIGINT", listener2);
  assertEquals(process.listeners("SIGINT").length, 0);
});

Deno.test(function processVersionsOwnProperty() {
  assert(Object.prototype.hasOwnProperty.call(process, "versions"));
});

Deno.test(function importedExecArgvTest() {
  assert(Array.isArray(importedExecArgv));
});

Deno.test(function importedExecPathTest() {
  assertEquals(importedExecPath, Deno.execPath());
});

Deno.test("process.cpuUsage()", () => {
  assert(process.cpuUsage.length === 1);
  const cpuUsage = process.cpuUsage();
  assert(typeof cpuUsage.user === "number");
  assert(typeof cpuUsage.system === "number");
  const a = process.cpuUsage();
  const b = process.cpuUsage(a);
  assert(a.user > b.user);
  assert(a.system > b.system);

  assertThrows(
    () => {
      // @ts-ignore TS2322
      process.cpuUsage({});
    },
    TypeError,
  );

  assertThrows(
    () => {
      // @ts-ignore TS2322
      process.cpuUsage({ user: "1", system: 2 });
    },
    TypeError,
  );
  assertThrows(
    () => {
      // @ts-ignore TS2322
      process.cpuUsage({ user: 1, system: "2" });
    },
    TypeError,
  );

  for (const invalidNumber of [-1, -Infinity, Infinity, NaN]) {
    assertThrows(
      () => {
        process.cpuUsage({ user: invalidNumber, system: 2 });
      },
      RangeError,
    );
    assertThrows(
      () => {
        process.cpuUsage({ user: 2, system: invalidNumber });
      },
      RangeError,
    );
  }
});

Deno.test("importedCpuUsage", () => {
  assert(importedCpuUsage === process.cpuUsage);
});

Deno.test("process.stdout.columns writable", () => {
  process.stdout.columns = 80;
  assertEquals(process.stdout.columns, 80);
});

Deno.test("getBuiltinModule", () => {
  assert(process.getBuiltinModule("fs"));
  assert(process.getBuiltinModule("node:fs"));
  assertEquals(process.getBuiltinModule("something"), undefined);
});

Deno.test("process.emitWarning() prints to stderr", async () => {
  using writeStub = stub(process.stderr, "write", () => true);
  const { promise, resolve } = Promise.withResolvers<void>();
  process.on("warning", () => resolve());

  process.emitWarning("This is a warning", {
    code: "TEST0001",
    detail: "This is some additional information",
  });

  await promise;

  const arg = writeStub.calls[0].args[0];
  assert(typeof arg === "string");
  assertMatch(
    arg,
    /\(node:\d+\) \[TEST0001\] Warning: This is a warning\nThis is some additional information\n/,
  );
});

Deno.test("process.emitWarning() does not print to stderr when it is deprecation warning and noDeprecation is set to true", async () => {
  using writeStub = stub(process.stderr, "write", () => true);

  // deno-lint-ignore no-explicit-any
  (process as any).noDeprecation = true; // Set noDeprecation to true
  process.emitWarning("This is a deprecation warning", {
    code: "TEST0002",
    detail: "This is some additional information",
    type: "DeprecationWarning",
  });

  await delay(10);

  assertEquals(writeStub.calls.length, 0);
  // deno-lint-ignore no-explicit-any
  (process as any).noDeprecation = false; // Reset noDeprecation
});

Deno.test("process.moduleLoadList", () => {
  // deno-lint-ignore no-explicit-any
  const moduleLoadList = (process as any).moduleLoadList;
  assert(Array.isArray(moduleLoadList));
  assertEquals(moduleLoadList.length, 0);
});

Deno.test({
  name: "process.setegid()",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    const originalEgid = getegid!();
    const groupName = getGroupNameFromSystem(originalEgid);

    // only assert that it doesn't throw
    setegid!(originalEgid);
    setegid!(groupName);
  },
});

Deno.test({
  name: "process.setegid() throws on invalid group",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    assertThrows(
      () => {
        setegid!("67?!");
      },
      "Group identifier does not exist: 67?!",
    );
  },
});

Deno.test({
  name: "process.setegid() should be undefined on unsupported platforms",
  ignore: Deno.build.os !== "windows" && Deno.build.os !== "android",
  fn() {
    assertEquals(process.setegid, undefined);
  },
});

Deno.test({
  name: "process.seteuid()",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    const originalEuid = geteuid!();
    const userName = getUserNameFromSystem(originalEuid);

    // calling with original euid to ensure it doesn't throw
    seteuid!(originalEuid);
    seteuid!(userName);
  },
});

Deno.test({
  name: "process.seteuid() throws on invalid user",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    assertThrows(
      () => {
        seteuid!("67?!");
      },
      "User identifier does not exist: 67?!",
    );
  },
});

Deno.test({
  name: "process.seteuid() should be undefined on unsupported platforms",
  ignore: Deno.build.os !== "windows" && Deno.build.os !== "android",
  fn() {
    assertEquals(process.seteuid, undefined);
  },
});

Deno.test({
  name: "process.setgid()",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    const originalGid = getgid!();
    const groupName = getGroupNameFromSystem(originalGid);

    // Calling with original gid to ensure it doesn't throw
    setgid!(originalGid);
    setgid!(groupName);
  },
});

Deno.test({
  name: "process.setgid() throws on invalid group",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    assertThrows(
      () => {
        setgid!("67?!");
      },
      "Group identifier does not exist: 67?!",
    );
  },
});

Deno.test({
  name: "process.setgid() should be undefined on unsupported platforms",
  ignore: Deno.build.os !== "windows" && Deno.build.os !== "android",
  fn() {
    assertEquals(process.setgid, undefined);
  },
});

Deno.test({
  name: "process.setuid()",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    const originalUid = getuid!();
    const userName = getUserNameFromSystem(originalUid);

    // Calling with original uid to ensure it doesn't throw
    setuid!(originalUid);
    setuid!(userName);
  },
});

Deno.test({
  name: "process.setuid() throws on invalid user",
  ignore: Deno.build.os === "windows" || Deno.build.os === "android",
  fn() {
    assertThrows(
      () => {
        setuid!("67?!");
      },
      "User identifier does not exist: 67?!",
    );
  },
});

Deno.test({
  name: "process.setuid() should be undefined on unsupported platforms",
  ignore: Deno.build.os !== "windows" && Deno.build.os !== "android",
  fn() {
    assertEquals(process.setuid, undefined);
  },
});
