// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrowsAsync,
} from "../../std/testing/asserts.ts";

Deno.test({
  name: "Deno.emit() - sources provided",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.ts",
      {
        sources: {
          "/foo.ts": `import * as bar from "./bar.ts";\n\nconsole.log(bar);\n`,
          "/bar.ts": `export const bar = "bar";\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files).sort();
    assert(keys[0].endsWith("/bar.ts.js"));
    assert(keys[1].endsWith("/bar.ts.js.map"));
    assert(keys[2].endsWith("/foo.ts.js"));
    assert(keys[3].endsWith("/foo.ts.js.map"));
  },
});

Deno.test({
  name: "Deno.emit() - no sources provided",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "./subdir/mod1.ts",
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files).sort();
    assertEquals(keys.length, 6);
    assert(keys[0].endsWith("cli/tests/subdir/mod1.ts.js"));
    assert(keys[1].endsWith("cli/tests/subdir/mod1.ts.js.map"));
  },
});

Deno.test({
  name: "Deno.emit() - compiler options effects emit",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.ts",
      {
        compilerOptions: {
          module: "amd",
          sourceMap: false,
        },
        sources: { "/foo.ts": `export const foo = "foo";` },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files);
    assertEquals(keys.length, 1);
    const key = keys[0];
    assert(key.endsWith("/foo.ts.js"));
    assert(files[key].startsWith("define("));
  },
});

Deno.test({
  name: "Deno.emit() - pass lib in compiler options",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "file:///foo.ts",
      {
        compilerOptions: {
          lib: ["dom", "es2018", "deno.ns"],
        },
        sources: {
          "file:///foo.ts": `console.log(document.getElementById("foo"));
          console.log(Deno.args);`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files).sort();
    assertEquals(keys, ["file:///foo.ts.js", "file:///foo.ts.js.map"]);
  },
});

Deno.test({
  name: "Deno.emit() - import maps",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "file:///a.ts",
      {
        importMap: {
          imports: {
            "b": "./b.ts",
          },
        },
        importMapPath: "file:///import-map.json",
        sources: {
          "file:///a.ts": `import * as b from "b"
            console.log(b);`,
          "file:///b.ts": `export const b = "b";`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files).sort();
    assertEquals(
      keys,
      [
        "file:///a.ts.js",
        "file:///a.ts.js.map",
        "file:///b.ts.js",
        "file:///b.ts.js.map",
      ],
    );
  },
});

Deno.test({
  name: "Deno.emit() - no check",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.ts",
      {
        check: false,
        sources: {
          "/foo.ts": `export enum Foo { Foo, Bar, Baz };\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 3);
    const keys = Object.keys(files).sort();
    assert(keys[0].endsWith("/foo.ts.js"));
    assert(keys[1].endsWith("/foo.ts.js.map"));
    assert(files[keys[0]].startsWith("export var Foo;"));
  },
});

Deno.test({
  name: "Deno.emit() - no check - config effects emit",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.ts",
      {
        check: false,
        compilerOptions: { removeComments: true },
        sources: {
          "/foo.ts":
            `/** This is JSDoc */\nexport enum Foo { Foo, Bar, Baz };\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 3);
    const keys = Object.keys(files).sort();
    assert(keys[0].endsWith("/foo.ts.js"));
    assert(keys[1].endsWith("/foo.ts.js.map"));
    assert(!files[keys[0]].includes("This is JSDoc"));
  },
});

Deno.test({
  name: "Deno.emit() - bundle esm - with sources",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.ts",
      {
        bundle: "esm",
        sources: {
          "/foo.ts": `export * from "./bar.ts";\n`,
          "/bar.ts": `export const bar = "bar";\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assertEquals(Object.keys(files), ["deno:///bundle.js"]);
    assert(files["deno:///bundle.js"].includes(`const bar1 = "bar"`));
  },
});

Deno.test({
  name: "Deno.emit() - bundle esm - no sources",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "./subdir/mod1.ts",
      {
        bundle: "esm",
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assertEquals(Object.keys(files), ["deno:///bundle.js"]);
    assert(files["deno:///bundle.js"].length);
  },
});

Deno.test({
  name: "Deno.emit() - bundle esm - include js modules",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.js",
      {
        bundle: "esm",
        sources: {
          "/foo.js": `export * from "./bar.js";\n`,
          "/bar.js": `export const bar = "bar";\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assertEquals(Object.keys(files), ["deno:///bundle.js"]);
    assert(files["deno:///bundle.js"].includes(`const bar1 = "bar"`));
  },
});

Deno.test({
  name: "Deno.emit() - generates diagnostics",
  async fn() {
    const { diagnostics, files } = await Deno.emit(
      "/foo.ts",
      {
        sources: {
          "/foo.ts": `document.getElementById("foo");`,
        },
      },
    );
    assertEquals(diagnostics.length, 1);
    const keys = Object.keys(files).sort();
    assert(keys[0].endsWith("/foo.ts.js"));
    assert(keys[1].endsWith("/foo.ts.js.map"));
  },
});

// See https://github.com/denoland/deno/issues/6908
Deno.test({
  name: "Deno.emit() - invalid syntax does not panic",
  async fn() {
    await assertThrowsAsync(async () => {
      await Deno.emit("/main.js", {
        sources: {
          "/main.js": `
            export class Foo {
              constructor() {
                console.log("foo");
              }
              export get() {
                console.log("bar");
              }
            }`,
        },
      });
    });
  },
});

Deno.test({
  name: 'Deno.emit() - allows setting of "importsNotUsedAsValues"',
  async fn() {
    const { diagnostics } = await Deno.emit("/a.ts", {
      sources: {
        "/a.ts": `import { B } from "./b.ts";
          const b: B = { b: "b" };`,
        "/b.ts": `export interface B {
          b:string;
        };`,
      },
      compilerOptions: {
        importsNotUsedAsValues: "error",
      },
    });
    assert(diagnostics);
    assertEquals(diagnostics.length, 1);
    assert(diagnostics[0].messageText);
    assert(diagnostics[0].messageText.includes("This import is never used"));
  },
});

Deno.test({
  name: "Deno.emit() - Unknown media type does not panic",
  async fn() {
    await assertThrowsAsync(async () => {
      await Deno.emit("https://example.com/foo", {
        sources: {
          "https://example.com/foo": `let foo: string = "foo";`,
        },
      });
    });
  },
});

Deno.test({
  name: "Deno.emit() - non-normalized specifier and source can compile",
  async fn() {
    const specifier = "https://example.com/foo//bar.ts";
    const { files } = await Deno.emit(specifier, {
      sources: {
        [specifier]: `export let foo: string = "foo";`,
      },
    });
    assertEquals(files[`${specifier}.js`], 'export let foo = "foo";\n');
    assert(typeof files[`${specifier}.js.map`] === "string");
  },
});
