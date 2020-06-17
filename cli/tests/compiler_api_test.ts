// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../../std/testing/asserts.ts";

Deno.test("compilerApiCompileSources", async function () {
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
});

Deno.test("compilerApiCompileNoSources", async function () {
  const [diagnostics, actual] = await Deno.compile("./subdir/mod1.ts");
  assert(diagnostics == null);
  assert(actual);
  const keys = Object.keys(actual);
  assertEquals(keys.length, 6);
  assert(keys[0].endsWith("print_hello.js.map"));
  assert(keys[1].endsWith("print_hello.js"));
});

Deno.test("compilerApiCompileOptions", async function () {
  const [diagnostics, actual] = await Deno.compile(
    "/foo.ts",
    {
      "/foo.ts": `export const foo = "foo";`,
    },
    {
      module: "amd",
      sourceMap: false,
    }
  );
  assert(diagnostics == null);
  assert(actual);
  assertEquals(Object.keys(actual), ["/foo.js"]);
  assert(actual["/foo.js"].startsWith("define("));
});

Deno.test("compilerApiCompileLib", async function () {
  const [diagnostics, actual] = await Deno.compile(
    "/foo.ts",
    {
      "/foo.ts": `console.log(document.getElementById("foo"));
        console.log(Deno.args);`,
    },
    {
      lib: ["dom", "es2018", "deno.ns"],
    }
  );
  assert(diagnostics == null);
  assert(actual);
  assertEquals(Object.keys(actual), ["/foo.js.map", "/foo.js"]);
});

Deno.test("compilerApiCompileTypes", async function () {
  const [diagnostics, actual] = await Deno.compile(
    "/foo.ts",
    {
      "/foo.ts": `console.log(Foo.bar);`,
    },
    {
      types: ["./subdir/foo_types.d.ts"],
    }
  );
  assert(diagnostics == null);
  assert(actual);
  assertEquals(Object.keys(actual), ["/foo.js.map", "/foo.js"]);
});

Deno.test("transpileOnlyApi", async function () {
  const actual = await Deno.transpileOnly({
    "foo.ts": `export enum Foo { Foo, Bar, Baz };\n`,
  });
  assert(actual);
  assertEquals(Object.keys(actual), ["foo.ts"]);
  assert(actual["foo.ts"].source.startsWith("export var Foo;"));
  assert(actual["foo.ts"].map);
});

Deno.test("transpileOnlyApiConfig", async function () {
  const actual = await Deno.transpileOnly(
    {
      "foo.ts": `export enum Foo { Foo, Bar, Baz };\n`,
    },
    {
      sourceMap: false,
      module: "amd",
    }
  );
  assert(actual);
  assertEquals(Object.keys(actual), ["foo.ts"]);
  assert(actual["foo.ts"].source.startsWith("define("));
  assert(actual["foo.ts"].map == null);
});

Deno.test("bundleApiSources", async function () {
  const [diagnostics, actual] = await Deno.bundle("/foo.ts", {
    "/foo.ts": `export * from "./bar.ts";\n`,
    "/bar.ts": `export const bar = "bar";\n`,
  });
  assert(diagnostics == null);
  assert(actual.includes(`__instantiate("foo")`));
  assert(actual.includes(`__exp["bar"]`));
});

Deno.test("bundleApiNoSources", async function () {
  const [diagnostics, actual] = await Deno.bundle("./subdir/mod1.ts");
  assert(diagnostics == null);
  assert(actual.includes(`__instantiate("mod1")`));
  assert(actual.includes(`__exp["printHello3"]`));
});

Deno.test("bundleApiConfig", async function () {
  const [diagnostics, actual] = await Deno.bundle(
    "/foo.ts",
    {
      "/foo.ts": `// random comment\nexport * from "./bar.ts";\n`,
      "/bar.ts": `export const bar = "bar";\n`,
    },
    {
      removeComments: true,
    }
  );
  assert(diagnostics == null);
  assert(!actual.includes(`random`));
});

Deno.test("bundleApiJsModules", async function () {
  const [diagnostics, actual] = await Deno.bundle("/foo.js", {
    "/foo.js": `export * from "./bar.js";\n`,
    "/bar.js": `export const bar = "bar";\n`,
  });
  assert(diagnostics == null);
  assert(actual.includes(`System.register("bar",`));
});

Deno.test("diagnosticsTest", async function () {
  const [diagnostics] = await Deno.compile("/foo.ts", {
    "/foo.ts": `document.getElementById("foo");`,
  });
  assert(Array.isArray(diagnostics));
  assert(diagnostics.length === 1);
});
