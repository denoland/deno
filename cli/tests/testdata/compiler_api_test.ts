// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
} from "../../../test_util/std/testing/asserts.ts";

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
    assert(keys[0].endsWith("subdir/mod1.ts.js"));
    assert(keys[1].endsWith("subdir/mod1.ts.js.map"));
  },
});

Deno.test({
  name: "Deno.emit() - data url",
  async fn() {
    const data =
      "data:application/javascript;base64,Y29uc29sZS5sb2coImhlbGxvIHdvcmxkIik7";
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(data);
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 0);
    const keys = Object.keys(files);
    assertEquals(keys.length, 1);
    assertEquals(keys[0], data);
    assertStringIncludes(files[keys[0]], 'console.log("hello world");');
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
  name: "Deno.emit() - type references can be loaded",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "file:///a.ts",
      {
        sources: {
          "file:///a.ts": `/// <reference types="./b.d.ts" />
          const b = new B();
          console.log(b.b);`,
          "file:///b.d.ts": `declare class B {
            b: string;
          }`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files).sort();
    assertEquals(keys, ["file:///a.ts.js", "file:///a.ts.js.map"]);
  },
});

Deno.test({
  name: "Deno.emit() - compilerOptions.types",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "file:///a.ts",
      {
        compilerOptions: {
          types: ["file:///b.d.ts"],
        },
        sources: {
          "file:///a.ts": `const b = new B();
          console.log(b.b);`,
          "file:///b.d.ts": `declare class B {
            b: string;
          }`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    const keys = Object.keys(files).sort();
    assertEquals(keys, ["file:///a.ts.js", "file:///a.ts.js.map"]);
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
  name: "Deno.emit() - allowSyntheticDefaultImports true by default",
  async fn() {
    const { diagnostics, files, ignoredOptions } = await Deno.emit(
      "file:///a.ts",
      {
        sources: {
          "file:///a.ts": `import b from "./b.js";\n`,
          "file:///b.js":
            `/// <reference types="./b.d.ts";\n\nconst b = "b";\n\nexport default b;\n`,
          "file:///b.d.ts": `declare const b: "b";\nexport = b;\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    const keys = Object.keys(files).sort();
    assertEquals(keys, [
      "file:///a.ts.js",
      "file:///a.ts.js.map",
      "file:///b.js",
    ]);
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
  name: "Deno.emit() - bundle as module script - with sources",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.ts",
      {
        bundle: "module",
        sources: {
          "/foo.ts": `export * from "./bar.ts";\n`,
          "/bar.ts": `export const bar = "bar";\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assertEquals(
      Object.keys(files).sort(),
      ["deno:///bundle.js", "deno:///bundle.js.map"].sort(),
    );
    assert(files["deno:///bundle.js"].includes(`const bar = "bar"`));
  },
});

Deno.test({
  name: "Deno.emit() - bundle as module script - no sources",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "./subdir/mod1.ts",
      {
        bundle: "module",
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assertEquals(
      Object.keys(files).sort(),
      ["deno:///bundle.js", "deno:///bundle.js.map"].sort(),
    );
    assert(files["deno:///bundle.js"].length);
  },
});

Deno.test({
  name: "Deno.emit() - bundle as module script - include js modules",
  async fn() {
    const { diagnostics, files, ignoredOptions, stats } = await Deno.emit(
      "/foo.js",
      {
        bundle: "module",
        sources: {
          "/foo.js": `export * from "./bar.js";\n`,
          "/bar.js": `export const bar = "bar";\n`,
        },
      },
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 0);
    assertEquals(
      Object.keys(files).sort(),
      ["deno:///bundle.js.map", "deno:///bundle.js"].sort(),
    );
    assert(files["deno:///bundle.js"].includes(`const bar = "bar"`));
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
    const { diagnostics } = await Deno.emit("/main.js", {
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
    assertEquals(diagnostics.length, 1);
    assert(
      diagnostics[0].messageText!.startsWith(
        "The module's source code could not be parsed: Unexpected token `get`. Expected * for generator, private key, identifier or async at file:",
      ),
    );
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
    await Deno.emit("https://example.com/foo", {
      sources: {
        "https://example.com/foo": `let foo: string = "foo";`,
      },
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

Deno.test({
  name: `Deno.emit() - bundle as classic script iife`,
  async fn() {
    const { diagnostics, files } = await Deno.emit("/a.ts", {
      bundle: "classic",
      sources: {
        "/a.ts": `import { b } from "./b.ts";
          console.log(b);`,
        "/b.ts": `export const b = "b";`,
      },
    });
    const ignoreDirecives = [
      "// deno-fmt-ignore-file",
      "// deno-lint-ignore-file",
      "// This code was bundled using `deno bundle` and it's not recommended to edit it manually",
      "",
      "",
    ].join("\n");
    assert(diagnostics);
    assertEquals(diagnostics.length, 0);
    assertEquals(Object.keys(files).length, 2);
    assert(
      files["deno:///bundle.js"].startsWith(
        ignoreDirecives + "(function() {\n",
      ),
    );
    assert(files["deno:///bundle.js"].endsWith("})();\n"));
    assert(files["deno:///bundle.js.map"]);
  },
});

Deno.test({
  name: `Deno.emit() - throws descriptive error when unable to load import map`,
  async fn() {
    await assertRejects(
      async () => {
        await Deno.emit("/a.ts", {
          bundle: "classic",
          sources: {
            "/a.ts": `console.log("hello");`,
          },
          importMapPath: "file:///import_map_does_not_exist.json",
        });
      },
      Error,
      "Unable to load 'file:///import_map_does_not_exist.json' import map",
    );
  },
});

Deno.test({
  name: `Deno.emit() - support source maps with bundle option`,
  async fn() {
    {
      const { diagnostics, files } = await Deno.emit("/a.ts", {
        bundle: "classic",
        sources: {
          "/a.ts": `import { b } from "./b.ts";
          console.log(b);`,
          "/b.ts": `export const b = "b";`,
        },
        compilerOptions: {
          inlineSourceMap: true,
          sourceMap: false,
        },
      });
      assert(diagnostics);
      assertEquals(diagnostics.length, 0);
      assertEquals(Object.keys(files).length, 1);
      assertStringIncludes(files["deno:///bundle.js"], "sourceMappingURL");
    }

    const { diagnostics, files } = await Deno.emit("/a.ts", {
      bundle: "classic",
      sources: {
        "/a.ts": `import { b } from "./b.ts";
        console.log(b);`,
        "/b.ts": `export const b = "b";`,
      },
    });
    assert(diagnostics);
    assertEquals(diagnostics.length, 0);
    assertEquals(Object.keys(files).length, 2);
    assert(files["deno:///bundle.js"]);
    assert(files["deno:///bundle.js.map"]);
  },
});

Deno.test({
  name: `Deno.emit() - graph errors as diagnostics`,
  ignore: Deno.build.os === "windows",
  async fn() {
    const { diagnostics } = await Deno.emit("/a.ts", {
      sources: {
        "/a.ts": `import { b } from "./b.ts";
        console.log(b);`,
      },
    });
    assert(diagnostics);
    assertEquals(diagnostics, [
      {
        category: 1,
        code: 2305,
        start: { line: 0, character: 9 },
        end: { line: 0, character: 10 },
        messageText:
          `Module '"deno:///missing_dependency.d.ts"' has no exported member 'b'.`,
        messageChain: null,
        source: null,
        sourceLine: 'import { b } from "./b.ts";',
        fileName: "file:///a.ts",
        relatedInformation: null,
      },
      {
        category: 1,
        code: 900001,
        start: null,
        end: null,
        messageText: 'Module not found "file:///b.ts".',
        messageChain: null,
        source: null,
        sourceLine: null,
        fileName: "file:///b.ts",
        relatedInformation: null,
      },
    ]);
    assert(
      Deno.formatDiagnostics(diagnostics).includes(
        'Module not found "file:///b.ts".',
      ),
    );
  },
});

Deno.test({
  name: "Deno.emit() - no check respects inlineSources compiler option",
  async fn() {
    const { files } = await Deno.emit(
      "file:///a.ts",
      {
        check: false,
        compilerOptions: {
          types: ["file:///b.d.ts"],
          inlineSources: true,
        },
        sources: {
          "file:///a.ts": `const b = new B();
          console.log(b.b);`,
          "file:///b.d.ts": `declare class B {
            b: string;
          }`,
        },
      },
    );
    const sourceMap: { sourcesContent?: string[] } = JSON.parse(
      files["file:///a.ts.js.map"],
    );
    assert(sourceMap.sourcesContent);
    assertEquals(sourceMap.sourcesContent.length, 1);
  },
});

Deno.test({
  name: "Deno.emit() - JSX import source pragma",
  async fn() {
    const { files } = await Deno.emit(
      "file:///a.tsx",
      {
        sources: {
          "file:///a.tsx": `/** @jsxImportSource https://example.com/jsx */

          export function App() {
            return (
              <div><></></div>
            );
          }`,
          "https://example.com/jsx/jsx-runtime": `export function jsx(
            _type,
            _props,
            _key,
            _source,
            _self,
          ) {}
          export const jsxs = jsx;
          export const jsxDEV = jsx;
          export const Fragment = Symbol("Fragment");
          console.log("imported", import.meta.url);
          `,
        },
      },
    );
    assert(files["file:///a.tsx.js"]);
    assert(
      files["file:///a.tsx.js"].startsWith(
        `import { Fragment as _Fragment, jsx as _jsx } from "https://example.com/jsx/jsx-runtime";\n`,
      ),
    );
  },
});

Deno.test({
  name: "Deno.emit() - JSX import source no pragma",
  async fn() {
    const { files } = await Deno.emit(
      "file:///a.tsx",
      {
        compilerOptions: {
          jsx: "react-jsx",
          jsxImportSource: "https://example.com/jsx",
        },
        sources: {
          "file:///a.tsx": `export function App() {
            return (
              <div><></></div>
            );
          }`,
          "https://example.com/jsx/jsx-runtime": `export function jsx(
            _type,
            _props,
            _key,
            _source,
            _self,
          ) {}
          export const jsxs = jsx;
          export const jsxDEV = jsx;
          export const Fragment = Symbol("Fragment");
          console.log("imported", import.meta.url);
          `,
        },
      },
    );
    assert(files["file:///a.tsx.js"]);
    assert(
      files["file:///a.tsx.js"].startsWith(
        `import { Fragment as _Fragment, jsx as _jsx } from "https://example.com/jsx/jsx-runtime";\n`,
      ),
    );
  },
});
