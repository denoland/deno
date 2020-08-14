// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrowsAsync,
} from "../../std/testing/asserts.ts";

Deno.test({
  name: "Deno.compile() - sources provided",
  async fn() {
    const [diagnostics, actual] = await Deno.compile("/foo.ts", {
      "/foo.ts": `import * as bar from "./bar.ts";\n\nconsole.log(bar);\n`,
      "/bar.ts": `export const bar = "bar";\n`,
    });
    assert(diagnostics == null);
    assert(actual);
    assertEquals(Object.keys(actual), [
      "/bar.js.map",
      "/bar.js",
      "/foo.js.map",
      "/foo.js",
    ]);
  },
});

Deno.test({
  name: "Deno.compile() - no sources provided",
  async fn() {
    const [diagnostics, actual] = await Deno.compile("./subdir/mod1.ts");
    assert(diagnostics == null);
    assert(actual);
    const keys = Object.keys(actual);
    assertEquals(keys.length, 6);
    assert(keys[0].endsWith("print_hello.js.map"));
    assert(keys[1].endsWith("print_hello.js"));
  },
});

Deno.test({
  name: "Deno.compile() - compiler options effects emit",
  async fn() {
    const [diagnostics, actual] = await Deno.compile(
      "/foo.ts",
      {
        "/foo.ts": `export const foo = "foo";`,
      },
      {
        module: "amd",
        sourceMap: false,
      },
    );
    assert(diagnostics == null);
    assert(actual);
    assertEquals(Object.keys(actual), ["/foo.js"]);
    assert(actual["/foo.js"].startsWith("define("));
  },
});

Deno.test({
  name: "Deno.compile() - pass lib in compiler options",
  async fn() {
    const [diagnostics, actual] = await Deno.compile(
      "/foo.ts",
      {
        "/foo.ts": `console.log(document.getElementById("foo"));
        console.log(Deno.args);`,
      },
      {
        lib: ["dom", "es2018", "deno.ns"],
      },
    );
    assert(diagnostics == null);
    assert(actual);
    assertEquals(Object.keys(actual), ["/foo.js.map", "/foo.js"]);
  },
});

Deno.test({
  name: "Deno.compile() - properly handles .d.ts files",
  async fn() {
    const [diagnostics, actual] = await Deno.compile(
      "/foo.ts",
      {
        "/foo.ts": `console.log(Foo.bar);`,
      },
      {
        types: ["./subdir/foo_types.d.ts"],
      },
    );
    assert(diagnostics == null);
    assert(actual);
    assertEquals(Object.keys(actual), ["/foo.js.map", "/foo.js"]);
  },
});

Deno.test({
  name: "Deno.transpileOnly()",
  async fn() {
    const actual = await Deno.transpileOnly({
      "foo.ts": `export enum Foo { Foo, Bar, Baz };\n`,
    });
    assert(actual);
    assertEquals(Object.keys(actual), ["foo.ts"]);
    assert(actual["foo.ts"].source.startsWith("export var Foo;"));
    assert(actual["foo.ts"].map);
  },
});

Deno.test({
  name: "Deno.transpileOnly() - config effects commit",
  async fn() {
    const actual = await Deno.transpileOnly(
      {
        "foo.ts": `export enum Foo { Foo, Bar, Baz };\n`,
      },
      {
        sourceMap: false,
        module: "amd",
      },
    );
    assert(actual);
    assertEquals(Object.keys(actual), ["foo.ts"]);
    assert(actual["foo.ts"].source.startsWith("define("));
    assert(actual["foo.ts"].map == null);
  },
});

Deno.test({
  name: "Deno.bundle() - sources passed",
  async fn() {
    const [diagnostics, actual] = await Deno.bundle("/foo.ts", {
      "/foo.ts": `export * from "./bar.ts";\n`,
      "/bar.ts": `export const bar = "bar";\n`,
    });
    assert(diagnostics == null);
    assert(actual.includes(`__instantiate("foo", false)`));
    assert(actual.includes(`__exp["bar"]`));
  },
});

Deno.test({
  name: "Deno.bundle() - no sources passed",
  async fn() {
    const [diagnostics, actual] = await Deno.bundle("./subdir/mod1.ts");
    assert(diagnostics == null);
    assert(actual.includes(`__instantiate("mod1", false)`));
    assert(actual.includes(`__exp["printHello3"]`));
  },
});

Deno.test({
  name: "Deno.bundle() - compiler config effects emit",
  async fn() {
    const [diagnostics, actual] = await Deno.bundle(
      "/foo.ts",
      {
        "/foo.ts": `// random comment\nexport * from "./bar.ts";\n`,
        "/bar.ts": `export const bar = "bar";\n`,
      },
      {
        removeComments: true,
      },
    );
    assert(diagnostics == null);
    assert(!actual.includes(`random`));
  },
});

Deno.test({
  name: "Deno.bundle() - JS Modules included",
  async fn() {
    const [diagnostics, actual] = await Deno.bundle("/foo.js", {
      "/foo.js": `export * from "./bar.js";\n`,
      "/bar.js": `export const bar = "bar";\n`,
    });
    assert(diagnostics == null);
    assert(actual.includes(`System.register("bar",`));
  },
});

Deno.test({
  name: "Deno.bundle - pre ES2017 uses ES5 loader",
  async fn() {
    const [diagnostics, actual] = await Deno.bundle(
      "/foo.ts",
      {
        "/foo.ts": `console.log("hello world!")\n`,
      },
      { target: "es2015" },
    );
    assert(diagnostics == null);
    assert(actual.includes(`var __awaiter = `));
  },
});

Deno.test({
  name: "runtime compiler APIs diagnostics",
  async fn() {
    const [diagnostics] = await Deno.compile("/foo.ts", {
      "/foo.ts": `document.getElementById("foo");`,
    });
    assert(Array.isArray(diagnostics));
    assert(diagnostics.length === 1);
  },
});

// See https://github.com/denoland/deno/issues/6908
Deno.test({
  name: "Deno.compile() - SWC diagnostics",
  async fn() {
    await assertThrowsAsync(async () => {
      await Deno.compile("main.js", {
        "main.js": `
      export class Foo {
        constructor() {
          console.log("foo");
        }
        export get() {
          console.log("bar");
        }
      }`,
      });
    });
  },
});
