// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
    const keys = Object.keys(actual).sort();
    assert(keys[0].endsWith("/bar.ts.js"));
    assert(keys[1].endsWith("/bar.ts.js.map"));
    assert(keys[2].endsWith("/foo.ts.js"));
    assert(keys[3].endsWith("/foo.ts.js.map"));
  },
});

Deno.test({
  name: "Deno.compile() - no sources provided",
  async fn() {
    const [diagnostics, actual] = await Deno.compile("./subdir/mod1.ts");
    assert(diagnostics == null);
    assert(actual);
    const keys = Object.keys(actual).sort();
    assertEquals(keys.length, 6);
    assert(keys[0].endsWith("cli/tests/subdir/mod1.ts.js"));
    assert(keys[1].endsWith("cli/tests/subdir/mod1.ts.js.map"));
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
    const keys = Object.keys(actual);
    assertEquals(keys.length, 1);
    const key = keys[0];
    assert(key.endsWith("/foo.ts.js"));
    assert(actual[key].startsWith("define("));
  },
});

Deno.test({
  name: "Deno.compile() - pass lib in compiler options",
  async fn() {
    const [diagnostics, actual] = await Deno.compile(
      "file:///foo.ts",
      {
        "file:///foo.ts": `console.log(document.getElementById("foo"));
        console.log(Deno.args);`,
      },
      {
        lib: ["dom", "es2018", "deno.ns"],
      },
    );
    assert(diagnostics == null);
    assert(actual);
    assertEquals(
      Object.keys(actual).sort(),
      ["file:///foo.ts.js", "file:///foo.ts.js.map"],
    );
  },
});

// TODO(@kitsonk) figure the "right way" to restore support for types
// Deno.test({
//   name: "Deno.compile() - properly handles .d.ts files",
//   async fn() {
//     const [diagnostics, actual] = await Deno.compile(
//       "/foo.ts",
//       {
//         "/foo.ts": `console.log(Foo.bar);`,
//         "/foo_types.d.ts": `declare namespace Foo {
//           const bar: string;
//         }`,
//       },
//       {
//         types: ["/foo_types.d.ts"],
//       },
//     );
//     assert(diagnostics == null);
//     assert(actual);
//     assertEquals(
//       Object.keys(actual).sort(),
//       ["file:///foo.ts.js", "file:///file.ts.js.map"],
//     );
//   },
// });

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
        "foo.ts": `/** This is JSDoc */\nexport enum Foo { Foo, Bar, Baz };\n`,
      },
      {
        removeComments: true,
      },
    );
    assert(actual);
    assertEquals(Object.keys(actual), ["foo.ts"]);
    assert(!actual["foo.ts"].source.includes("This is JSDoc"));
    assert(actual["foo.ts"].map);
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
    assert(actual.includes(`const bar = "bar"`));
  },
});

Deno.test({
  name: "Deno.bundle() - no sources passed",
  async fn() {
    const [diagnostics, actual] = await Deno.bundle("./subdir/mod1.ts");
    assert(diagnostics == null);
    assert(actual.length);
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
    assert(actual.includes(`const bar = "bar"`));
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
      await Deno.compile("/main.js", {
        "/main.js": `
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

Deno.test({
  name: `Deno.compile() - Allows setting of "importsNotUsedAsValues"`,
  async fn() {
    const [diagnostics] = await Deno.compile("/a.ts", {
      "/a.ts": `import { B } from "./b.ts";
        const b: B = { b: "b" };
      `,
      "/b.ts": `export interface B {
        b: string;
      };
      `,
    }, {
      importsNotUsedAsValues: "error",
    });
    assert(diagnostics);
    assertEquals(diagnostics.length, 1);
    assert(diagnostics[0].messageText);
    assert(diagnostics[0].messageText.includes("This import is never used"));
  },
});
