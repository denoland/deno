// deno-lint-ignore-file no-undef
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import process, { argv, env } from "node:process";
import {
  assert,
  assertEquals,
  assertFalse,
  assertObjectMatch,
  assertStrictEquals,
  assertThrows,
} from "../../../test_util/std/testing/asserts.ts";
import { stripColor } from "../../../test_util/std/fmt/colors.ts";
import { deferred } from "../../../test_util/std/async/deferred.ts";
import * as path from "../../../test_util/std/path/mod.ts";
import { delay } from "../../../test_util/std/async/delay.ts";

const testDir = new URL(".", import.meta.url);

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
    assertEquals(typeof process.platform, "string");
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
    assertEquals(typeof process.arch, "string");
    if (Deno.build.arch == "x86_64") {
      assertEquals(process.arch, "x64");
    } else if (Deno.build.arch == "aarch64") {
      assertEquals(process.arch, "arm64");
    } else {
      throw new Error("unreachable");
    }
  },
});

Deno.test({
  name: "process.pid",
  fn() {
    assertEquals(typeof process.pid, "number");
    assertEquals(process.pid, Deno.pid);
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
        "--unstable",
        "./testdata/process_exit.ts",
      ],
      cwd,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(stripColor(decoder.decode(stdout).trim()), "1\n2");
  },
});

Deno.test({
  name: "process.on signal",
  ignore: Deno.build.os == "windows",
  async fn() {
    const process = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        `
        import process from "node:process";
        setInterval(() => {}, 1000);
        process.on("SIGINT", () => {
          console.log("foo");
        });
        `,
      ],
      stdout: "piped",
      stderr: "null",
    }).spawn();
    await delay(500);
    for (const _ of Array(3)) {
      process.kill("SIGINT");
      await delay(20);
    }
    await delay(20);
    process.kill("SIGTERM");
    const output = await process.output();
    assertEquals(new TextDecoder().decode(output.stdout), "foo\nfoo\nfoo\n");
  },
});

Deno.test({
  name: "process.off signal",
  ignore: Deno.build.os == "windows",
  async fn() {
    const process = new Deno.Command(Deno.execPath(), {
      args: [
        "eval",
        `
        import process from "node:process";
        setInterval(() => {}, 1000);
        const listener = () => {
          console.log("foo");
          process.off("SIGINT")
        };
        process.on("SIGINT", listener);
        `,
      ],
      stdout: "piped",
      stderr: "null",
    }).spawn();
    await delay(500);
    for (const _ of Array(3)) {
      try {
        process.kill("SIGINT");
      } catch { /* should die after the first one */ }
      await delay(20);
    }
    await delay(20);
    try {
      process.kill("SIGTERM");
    } catch { /* should be dead, avoid hanging just in case */ }
    const output = await process.output();
    assertEquals(new TextDecoder().decode(output.stdout), "foo\n");
  },
});

Deno.test({
  name: "process.on SIGBREAK doesn't throw",
  fn() {
    const listener = () => {};
    process.on("SIGBREAK", listener);
    process.off("SIGBREAK", listener);
  },
});

Deno.test({
  name: "process.on SIGTERM doesn't throw on windows",
  ignore: Deno.build.os !== "windows",
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
  },
});

Deno.test({
  name: "process.env requires scoped env permission",
  permissions: { env: ["FOO"] },
  fn() {
    Deno.env.set("FOO", "1");
    assert("FOO" in process.env);
    assertFalse("BAR" in process.env);
    assert(Object.hasOwn(process.env, "FOO"));
    assertFalse(Object.hasOwn(process.env, "BAR"));
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
  name: "process.stdin",
  fn() {
    assertEquals(process.stdin.fd, Deno.stdin.rid);
    assertEquals(process.stdin.isTTY, Deno.isatty(Deno.stdin.rid));
  },
});

Deno.test({
  name: "process.stdin readable with a TTY",
  // TODO(PolarETech): Run this test even in non tty environment
  ignore: !Deno.isatty(Deno.stdin.rid),
  async fn() {
    const promise = deferred();
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
        promise.resolve();
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
    const expected = ["16384", "foo", "bar", "null", "end"];
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
    const expected = ["16384", "foo", "bar", "null", "end"];
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
    const expected = ["16384", "null", "end"];
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
    assertEquals(process.stdout.fd, Deno.stdout.rid);
    const isTTY = Deno.isatty(Deno.stdout.rid);
    assertEquals(process.stdout.isTTY, isTTY);
    const consoleSize = isTTY ? Deno.consoleSize() : undefined;
    assertEquals(process.stdout.columns, consoleSize?.columns);
    assertEquals(process.stdout.rows, consoleSize?.rows);
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
  },
});

Deno.test({
  name: "process.stderr",
  fn() {
    assertEquals(process.stderr.fd, Deno.stderr.rid);
    const isTTY = Deno.isatty(Deno.stderr.rid);
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
  assert(process.exitCode === undefined);
  process.exitCode = 127;
  assert(process.exitCode === 127);
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

Deno.test({
  name: "process.exit",
  async fn() {
    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--quiet",
        "--unstable",
        "./testdata/process_exit2.ts",
      ],
      cwd: testDir,
    });
    const { stdout } = await command.output();

    const decoder = new TextDecoder();
    assertEquals(stripColor(decoder.decode(stdout).trim()), "exit");
  },
});
