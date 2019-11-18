import { test } from "../testing/mod.ts";
import { assert, assertThrows, assertEquals } from "../testing/asserts.ts";
import * as process from "./process.ts";

// NOTE: Deno.cwd() and Deno.chdir() currently require --allow-env
// (Also Deno.env() requires --allow-env but it's more obvious)

test({
  name: "process.cwd and process.chdir success",
  fn() {
    // this should be run like other tests from directory up
    assert(process.cwd().match(/\Wstd$/));
    process.chdir("node");
    assert(process.cwd().match(/\Wnode$/));
    process.chdir("..");
    assert(process.cwd().match(/\Wstd$/));
  }
});

test({
  name: "process.chdir failure",
  fn() {
    assertThrows(
      () => {
        process.chdir("non-existent-directory-name");
      },
      Deno.DenoError,
      "No such file"
    );
  }
});

test({
  name: "process.version",
  fn() {
    assertEquals(typeof process, "object");
    assertEquals(typeof process.version, "string");
    assertEquals(typeof process.versions, "object");
    assertEquals(typeof process.versions.node, "string");
  }
});

test({
  name: "process.platform",
  fn() {
    assertEquals(typeof process.platform, "string");
  }
});

test({
  name: "process.arch",
  fn() {
    assertEquals(typeof process.arch, "string");
    // TODO(rsp): make sure that the arch strings should be the same in Node and Deno:
    assertEquals(process.arch, Deno.build.arch);
  }
});

test({
  name: "process.pid",
  fn() {
    assertEquals(typeof process.pid, "number");
    assertEquals(process.pid, Deno.pid);
  }
});

test({
  name: "process.on",
  fn() {
    assertEquals(typeof process.on, "function");
    assertThrows(
      () => {
        process.on("uncaughtException", (_err: Error) => {});
      },
      Error,
      "unimplemented"
    );
  }
});

test({
  name: "process.argv",
  fn() {
    assert(Array.isArray(process.argv));
    assert(
      process.argv.filter(x => x.match(/process_test[.]ts$/)).length > 0,
      `file name process_test.ts in process.argv = ${JSON.stringify(
        process.argv
      )}`
    );
  }
});

test({
  name: "process.env",
  fn() {
    assertEquals(typeof process.env.PATH, "string");
  }
});
