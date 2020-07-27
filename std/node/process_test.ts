// deno-lint-ignore-file no-undef
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, assertThrows } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import * as all from "./process.ts";
import { argv, env } from "./process.ts";
import { delay } from "../async/delay.ts";
import "./global.ts";

// NOTE: Deno.execPath() (and thus process.argv) currently requires --allow-env
// (Also Deno.env.toObject() (and process.env) requires --allow-env but it's more obvious)

Deno.test({
  name: "process exports are as they should be",
  fn() {
    // * should be the same as process, default, and globalThis.process
    // without the export aliases, and with properties that are not standalone
    const allKeys = new Set<string>(Object.keys(all));
    // without { process } for deno b/c
    allKeys.delete("process");
    // without esm default
    allKeys.delete("default");
    // with on, stdin, stderr, and stdout, which is not exported via *
    allKeys.add("on");
    allKeys.add("stdin");
    allKeys.add("stderr");
    allKeys.add("stdout");
    const allStr = Array.from(allKeys).sort().join(" ");
    assertEquals(Object.keys(all.default).sort().join(" "), allStr);
    assertEquals(Object.keys(all.process).sort().join(" "), allStr);
    assertEquals(Object.keys(process).sort().join(" "), allStr);
  },
});

Deno.test({
  name: "process.cwd and process.chdir success",
  fn() {
    assertEquals(process.cwd(), Deno.cwd());

    const currentDir = Deno.cwd(); // to unchange current directory after this test

    const moduleDir = path.parent(path.fromFileUrl(import.meta.url));
    process.chdir(path.resolve(moduleDir, ".."));

    assert(process.cwd().match(/\Wstd$/));
    process.chdir("node");
    assert(process.cwd().match(/\Wnode$/));
    process.chdir("..");
    assert(process.cwd().match(/\Wstd$/));
    process.chdir(currentDir); // to unchange current directory after this test
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
  },
});

Deno.test({
  name: "process.platform",
  fn() {
    assertEquals(typeof process.platform, "string");
  },
});

Deno.test({
  name: "process.arch",
  fn() {
    assertEquals(typeof process.arch, "string");
    // TODO(rsp): make sure that the arch strings should be the same in Node and Deno:
    assertEquals(process.arch, Deno.build.arch);
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
  fn() {
    assertEquals(typeof process.on, "function");
    assertThrows(
      () => {
        process.on("uncaughtException", (_err: Error) => {});
      },
      Error,
      "implemented",
    );
  },
});

Deno.test({
  name: "process.argv",
  fn() {
    assert(Array.isArray(process.argv));
    assert(Array.isArray(argv));
    assert(
      process.argv[0].match(/[^/\\]*deno[^/\\]*$/),
      "deno included in the file name of argv[0]",
    );
    // we cannot test for anything else (we see test runner arguments here)
  },
});

Deno.test({
  name: "process.env",
  fn() {
    assertEquals(typeof process.env.PATH, "string");
    assertEquals(typeof env.PATH, "string");
  },
});

Deno.test({
  name: "process.stdin",
  fn() {
    assertEquals(typeof process.stdin.fd, "number");
    assertEquals(process.stdin.fd, Deno.stdin.rid);
    // TODO(jayhelton) Uncomment out this assertion once PTY is supported
    //assert(process.stdin.isTTY);
  },
});

Deno.test({
  name: "process.stdout",
  fn() {
    assertEquals(typeof process.stdout.fd, "number");
    assertEquals(process.stdout.fd, Deno.stdout.rid);
    // TODO(jayhelton) Uncomment out this assertion once PTY is supported
    // assert(process.stdout.isTTY);
  },
});

Deno.test({
  name: "process.stderr",
  fn() {
    assertEquals(typeof process.stderr.fd, "number");
    assertEquals(process.stderr.fd, Deno.stderr.rid);
    // TODO(jayhelton) Uncomment out this assertion once PTY is supported
    // assert(process.stderr.isTTY);
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
