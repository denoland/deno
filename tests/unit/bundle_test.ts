// Copyright 2018-2025 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertFalse,
  assertStringIncludes,
  unindent,
} from "./test_util.ts";

import { basename, join, toFileUrl } from "@std/path";

class TempDir implements AsyncDisposable, Disposable {
  private path: string;
  constructor(options?: Deno.MakeTempOptions) {
    this.path = Deno.makeTempDirSync(options);
  }

  async [Symbol.asyncDispose]() {
    await Deno.remove(this.path, { recursive: true });
  }

  [Symbol.dispose]() {
    Deno.removeSync(this.path, { recursive: true });
  }

  join(path: string) {
    return join(this.path, path);
  }
}

class TempFile implements AsyncDisposable, Disposable {
  #path: string;
  constructor(options?: Deno.MakeTempOptions) {
    this.#path = Deno.makeTempFileSync(options);
  }

  async [Symbol.asyncDispose]() {
    await Deno.remove(this.#path);
  }

  [Symbol.dispose]() {
    Deno.removeSync(this.#path);
  }

  get path() {
    return this.#path;
  }
}

Deno.test("bundle: basic in-memory bundle succeeds and returns content", async () => {
  using dir = new TempDir();
  const entry = dir.join("index.ts");
  const dep = dir.join("mod.ts");

  await Deno.writeTextFile(
    dep,
    [
      "export function greet(name: string) {",
      "  return `hello ${name}`;",
      "}",
    ].join("\n"),
  );
  await Deno.writeTextFile(
    entry,
    [
      "import { greet } from './mod.ts';",
      "console.log(greet('world'));",
    ].join("\n"),
  );

  const result = await Deno.bundle({
    entrypoints: [entry],
    // keep readable to assert on content
    minify: false,
    write: false,
  });

  assertEquals(result.success, true);
  assertEquals(result.errors.length, 0);
  assert(Array.isArray(result.warnings));
  assert(result.outputFiles !== undefined);
  const withContent = result.outputFiles!.filter((f) => !!f.contents);
  assert(withContent.length >= 1);
  const content = withContent[0].text();
  // should contain the string literal from the source
  assertStringIncludes(content, "hello ");
  // stripped of TS types
  assertFalse(content.includes(": string"));
});

Deno.test("bundle: write to outputDir omits outputFiles and writes files", async () => {
  using dir = new TempDir();
  const srcDir = dir.join("src");
  const outDir = dir.join("dist");
  await Deno.mkdir(srcDir, { recursive: true });
  const entry = join(srcDir, "main.ts");
  const dep = join(srcDir, "util.ts");

  await Deno.writeTextFile(
    dep,
    unindent`
      export const msg: string = 'Hello bundle write';
      export function upper(s: string) { return s.toUpperCase(); }
    `,
  );
  await Deno.writeTextFile(
    entry,
    unindent`
      import { msg, upper } from './util.ts';
      console.log(upper(msg));
    `,
  );

  const result = await Deno.bundle({
    entrypoints: [entry],
    outputDir: outDir,
    // default write is true when outputDir/outputPath is set
    // but be explicit here
    write: true,
    minify: false,
  });

  assertEquals(result.success, true);
  assertEquals(result.errors.length, 0);
  // when writing, the provider returns `null` for outputFiles currently
  assert(result.outputFiles == null);

  // verify a JS file was written to the output directory
  const files = [] as string[];
  for await (const e of Deno.readDir(outDir)) {
    if (e.isFile && e.name.endsWith(".js")) files.push(e.name);
  }
  assert(files.length >= 1);
  // read first file and check expected content present
  const outJsPath = join(outDir, files[0]);
  const outContent = await Deno.readTextFile(outJsPath);
  assertStringIncludes(outContent, "Hello bundle write");
});

Deno.test("bundle: minify produces smaller output", async () => {
  using dir = new TempDir();
  const entry = dir.join("index.ts");
  const dep = dir.join("calc.ts");

  await Deno.writeTextFile(
    dep,
    unindent`
      export function add(a: number, b: number) {
        /* lots of spacing and comments to be minified */
        const sum = a + b;  // trailing comment
        return sum;
      }
    `,
  );
  await Deno.writeTextFile(
    entry,
    unindent`
      import { add } from './calc.ts';
      console.log(add(  1,   2));
    `,
  );

  const normal = await Deno.bundle({
    entrypoints: [entry],
    minify: false,
    write: false,
  });
  assertEquals(normal.success, true);
  const normalJs = normal.outputFiles!.find((f) => !!f.contents)!;

  const minified = await Deno.bundle({
    entrypoints: [entry],
    minify: true,
    write: false,
  });
  assertEquals(minified.success, true);
  const minJs = minified.outputFiles!.find((f) => !!f.contents)!;

  assert(minJs.text().length < normalJs.text().length);
});

Deno.test("bundle: code splitting with multiple entrypoints", async () => {
  using dir = new TempDir();
  const shared = dir.join("shared.ts");
  const a = dir.join("a.ts");
  const b = dir.join("b.ts");

  await Deno.writeTextFile(
    shared,
    unindent`
    export const shared = 'shared chunk';
  `,
  );
  await Deno.writeTextFile(
    a,
    unindent`
      import { shared } from './shared.ts';
      console.log('a', shared);
    `,
  );
  await Deno.writeTextFile(
    b,
    unindent`
      import { shared } from './shared.ts';
      console.log('b', shared);
    `,
  );

  const outDir = dir.join("dist");
  const result = await Deno.bundle({
    entrypoints: [a, b],
    codeSplitting: true,
    // esbuild requires an output directory when splitting
    outputDir: outDir,
    write: false,
    minify: false,
  });

  assertEquals(result.success, true);
  assert(result.outputFiles !== undefined);
  const jsFiles = result.outputFiles!.filter((f) => !!f.contents);
  // 2 entries + at least 1 shared chunk
  assert(jsFiles.length >= 3);
});

Deno.test("bundle: inline sourcemap is present", async () => {
  using dir = new TempDir();
  const entry = dir.join("index.ts");
  await Deno.writeTextFile(entry, "export const x = 1;\n");

  const result = await Deno.bundle({
    entrypoints: [entry],
    sourcemap: "inline",
    write: false,
    minify: false,
  });

  assertEquals(result.success, true);
  const js = result.outputFiles!.find((f) => !!f.contents)!;
  assertStringIncludes(
    js.text(),
    "sourceMappingURL=data:application/json;base64",
  );
});

Deno.test("bundle: missing entrypoint rejects", async () => {
  using dir = new TempDir();
  const missing = dir.join("does_not_exist.ts");

  let threw = false;
  try {
    await Deno.bundle({
      entrypoints: [missing],
      write: false,
    });
  } catch (_e) {
    threw = true;
  }
  assert(threw);
});

Deno.test("bundle: returns errors for unresolved import", async () => {
  using dir = new TempDir();
  const entry = dir.join("main.ts");
  // entry exists, but imports a non-existent module
  await Deno.writeTextFile(
    entry,
    unindent`
      import './missing.ts';
      export const value = 42;
    `,
  );

  const result = await Deno.bundle({
    entrypoints: [entry],
    write: false,
    minify: false,
  });

  assertEquals(result.success, false);
  assert(result.errors.length > 0);
  // should reference the missing import path in one of the error messages
  const texts = result.errors.map((e) => e.text).join("\n");
  assertStringIncludes(texts, "missing.ts");
});

// deno-lint-ignore no-explicit-any
async function evalEsmString(code: string): Promise<any> {
  await using file = new TempFile({ suffix: ".js" });
  Deno.writeTextFileSync(file.path, code);
  return await import(toFileUrl(file.path).toString());
}

Deno.test("bundle: replaces require shim when platform is deno", async () => {
  using dir = new TempDir();
  const entry = dir.join("index.cjs");
  const input = unindent`
    const sep = require("node:path").sep;
    module.exports = ["good", sep.length];
  `;
  await Deno.writeTextFile(entry, input);

  const result = await Deno.bundle({
    entrypoints: [entry],
    platform: "deno",
    write: false,
  });

  assertEquals(result.success, true);
  const js = result.outputFiles!.find((f) => !!f.contents)!;

  const output = await evalEsmString(js.text());
  assertEquals(output.default, ["good", 1]);
});

Deno.test("bundle: html works", async () => {
  using dir = new TempDir();
  const entry = dir.join("index.html");
  const input = unindent`
    <html>
      <body>
        <script src="./index.ts"></script>
      </body>
    </html>
  `;
  const script = dir.join("index.ts");
  const scriptInput = unindent`
    console.log("hello");
    document.body.innerHTML = "hello";
  `;
  const outDir = dir.join("dist");

  await Deno.writeTextFile(entry, input);
  await Deno.writeTextFile(script, scriptInput);

  const result = await Deno.bundle({
    entrypoints: [entry],
    outputDir: outDir,
    write: false,
  });

  const js = result.outputFiles!.find((f) =>
    !!f.contents && f.path.endsWith(".js")
  );
  if (!js) {
    throw new Error("No JS file found");
  }

  const html = result.outputFiles!.find((f) =>
    !!f.contents && f.path.endsWith(".html")
  )!;
  if (!html) {
    throw new Error("No HTML file found");
  }

  assert(result.success);
  assertEquals(result.errors.length, 0);
  assertEquals(result.outputFiles!.length, 2);

  const jsFileName = basename(js.path);

  assertStringIncludes(html.text(), `src="./${jsFileName}`);

  assertStringIncludes(js.text(), "innerHTML");
  assertStringIncludes(js.text(), "hello");
});
