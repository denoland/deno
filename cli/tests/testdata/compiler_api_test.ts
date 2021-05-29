// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrowsAsync,
} from "../../../test_util/std/testing/asserts.ts";

Deno.test({
  name: "Deno.emit() - sources provided",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 2);
    modules.sort((m1, m2) => (m1.specifier > m2.specifier) ? 1 : -1);
    const [module0, module1] = modules;
    assert(module0.specifier.endsWith("/bar.ts"));
    assert(!("error" in module0));
    assert(module0.map != null);
    assert(module1.specifier.endsWith("/foo.ts"));
    assert(!("error" in module1));
    assert(module1.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - no sources provided",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
      "./subdir/mod1.ts",
    );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assertEquals(modules.length, 3);
    modules.sort((m1, m2) => (m1.specifier > m2.specifier) ? 1 : -1);
    const module0 = modules[0];
    assert(module0.specifier.endsWith("subdir/mod1.ts"));
    assert(!("error" in module0));
    assert(module0.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - compiler options effects emit",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 1);
    const module = modules[0];
    assert(module.specifier.endsWith("/foo.ts"));
    assert(!("error" in module));
    assert(module.code.startsWith("define("));
    assert(module.map == null);
  },
});

Deno.test({
  name: "Deno.emit() - pass lib in compiler options",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 1);
    const module = modules[0];
    assertEquals(module.specifier, "file:///foo.ts");
    assert(!("error" in module));
    assert(module.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - type references can be loaded",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 1);
    const module = modules[0];
    assertEquals(module.specifier, "file:///a.ts");
    assert(!("error" in module));
    assert(module.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - compilerOptions.types",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    const module = modules[0];
    assertEquals(module.specifier, "file:///a.ts");
    assert(!("error" in module));
    assert(module.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - import maps",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 2);
    modules.sort((m1, m2) => (m1.specifier > m2.specifier) ? 1 : -1);
    const [module0, module1] = modules;
    assertEquals(module0.specifier, "file:///a.ts");
    assert(!("error" in module0));
    assert(module0.map != null);
    assertEquals(module1.specifier, "file:///b.ts");
    assert(!("error" in module1));
    assert(module1.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - no check",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 1);
    const module = modules[0];
    assert(module.specifier.endsWith("/foo.ts"));
    assert(!("error" in module));
    assert(module.code.startsWith("export var Foo;"));
    assert(module.map != null);
  },
});

Deno.test({
  name: "Deno.emit() - no check - config effects emit",
  async fn() {
    const { diagnostics, modules, ignoredOptions, stats } = await Deno.emit(
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
    assertEquals(modules.length, 1);
    const module = modules[0];
    assert(module.specifier.endsWith("/foo.ts"));
    assert(!("error" in module));
    assert(!module.code.includes("This is JSDoc"));
    assert(module.map != null);
  },
});

Deno.test({
  name: "Deno.emitBundle() - bundle as module script - with sources",
  async fn() {
    const { diagnostics, code, map, ignoredOptions, stats } = await Deno
      .emitBundle(
        "/foo.ts",
        {
          sources: {
            "/foo.ts": `export * from "./bar.ts";\n`,
            "/bar.ts": `export const bar = "bar";\n`,
          },
        },
      );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assert(code.includes(`const bar1 = "bar"`));
    assert(map != null);
  },
});

Deno.test({
  name: "Deno.emitBundle() - bundle as module script - no sources",
  async fn() {
    const { diagnostics, code, map, ignoredOptions, stats } = await Deno
      .emitBundle("./subdir/mod1.ts");
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assert(code.includes("returnsHi"));
    assert(map != null);
  },
});

Deno.test({
  name: "Deno.emitBundle() - bundle as module script - include js modules",
  async fn() {
    const { diagnostics, code, map, ignoredOptions, stats } = await Deno
      .emitBundle(
        "/foo.js",
        {
          sources: {
            "/foo.js": `export * from "./bar.js";\n`,
            "/bar.js": `export const bar = "bar";\n`,
          },
        },
      );
    assertEquals(diagnostics.length, 0);
    assert(!ignoredOptions);
    assertEquals(stats.length, 12);
    assert(code.includes(`const bar1 = "bar"`));
    assert(map != null);
  },
});

Deno.test({
  name: "Deno.emit() - generates diagnostics",
  async fn() {
    const { diagnostics, modules } = await Deno.emit(
      "/foo.ts",
      {
        sources: {
          "/foo.ts": `document.getElementById("foo");`,
        },
      },
    );
    assertEquals(diagnostics.length, 1);
    assertEquals(modules.length, 1);
    const module = modules[0];
    assert(module.specifier.endsWith("/foo.ts"));
    assert(!("error" in module));
    assert(module.map != null);
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
    const { modules } = await Deno.emit(specifier, {
      sources: {
        [specifier]: `export let foo: string = "foo";`,
      },
    });
    assertEquals(modules.length, 1);
    const module = modules[0];
    assertEquals(module.specifier, specifier);
    assert(!("error" in module));
    assertEquals(module.code, 'export let foo = "foo";\n');
    assert(module.map != null);
  },
});

Deno.test({
  name: `Deno.emitBundle() - bundle as classic script iife`,
  async fn() {
    const { diagnostics, code, map } = await Deno.emitBundle("/a.ts", {
      sources: {
        "/a.ts": `import { b } from "./b.ts";
          console.log(b);`,
        "/b.ts": `export const b = "b";`,
      },
      type: "classic",
    });
    assert(diagnostics);
    assertEquals(diagnostics.length, 0);
    assert(code.startsWith("(function() {\n"));
    assert(code.endsWith("})();\n"));
    assert(map != null);
  },
});

Deno.test({
  name: `Deno.emit() - throws descriptive error when unable to load import map`,
  async fn() {
    await assertThrowsAsync(
      async () => {
        await Deno.emit("/a.ts", {
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
  name: `Deno.emitBundle() - support source maps`,
  async fn() {
    {
      const { diagnostics, code, map } = await Deno.emitBundle("/a.ts", {
        sources: {
          "/a.ts": `import { b } from "./b.ts";
          console.log(b);`,
          "/b.ts": `export const b = "b";`,
        },
        compilerOptions: {
          inlineSourceMap: true,
          sourceMap: false,
        },
        type: "classic",
      });
      assert(diagnostics);
      assertEquals(diagnostics.length, 0);
      assertStringIncludes(code, "sourceMappingURL");
      assert(map == null);
    }

    const { diagnostics, map } = await Deno.emitBundle("/a.ts", {
      sources: {
        "/a.ts": `import { b } from "./b.ts";
        console.log(b);`,
        "/b.ts": `export const b = "b";`,
      },
      type: "classic",
    });
    assert(diagnostics);
    assertEquals(diagnostics.length, 0);
    assert(map != null);
  },
});

Deno.test({
  name: `Deno.emit() - graph errors as error entries`,
  ignore: Deno.build.os === "windows",
  async fn() {
    const { modules } = await Deno.emit("/a.ts", {
      sources: {
        "/a.ts": `import { b } from "./b.ts";
        console.log(b);`,
      },
    });
    for (const module of modules) {
      if (module.specifier == "file:///a.ts") {
        assert(!("error" in module));
        assertStringIncludes(module.code, "import");
      } else if (module.specifier == "file:///b.ts") {
        assert("error" in module);
        assertEquals(
          module.error,
          "Unable to find specifier in sources: file:///b.ts",
        );
      } else {
        throw new Error("Unreachable: There should be no other specifier.");
      }
    }
  },
});

Deno.test({
  name: `Deno.emit() - data URLs`,
  ignore: Deno.build.os === "windows",
  async fn() {
    const specifier2 = `data:application/typescript;base64,${
      btoa(`console.log("hello");`)
    }`;
    const specifier1 = `data:application/typescript;base64,${
      btoa(`import "${specifier2}";`)
    }`;
    const { modules } = await Deno.emit(specifier1);
    for (const module of modules) {
      if (module.specifier == specifier1) {
        assert(!("error" in module));
        assertStringIncludes(module.code, "import");
        assertStringIncludes(module.code, specifier2);
      } else if (module.specifier == specifier2) {
        assert(!("error" in module));
        assertStringIncludes(module.code, "console.log");
        assertStringIncludes(module.code, "hello");
      } else {
        throw new Error("Unreachable: There should be no other specifier.");
      }
    }
  },
});
