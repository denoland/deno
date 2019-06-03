// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { join } from "../fs/path.ts";
import { EOL } from "../fs/path/constants.ts";
import { assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import { xrun } from "./util.ts";
import { copy, emptyDir } from "../fs/mod.ts";
const { readAll, execPath } = Deno;

const decoder = new TextDecoder();

async function run(
  args: string[]
): Promise<{ stdout: string; code: number | undefined }> {
  const p = xrun({ args, stdout: "piped" });

  const stdout = decoder.decode(await readAll(p.stdout!));
  const { code } = await p.status();

  return { stdout, code };
}

const cmd = [
  execPath,
  "run",
  "--allow-run",
  "--allow-write",
  "--allow-read",
  "prettier/main.ts"
];
const testdata = join("prettier", "testdata");

function normalizeOutput(output: string): string {
  return output
    .replace(/\r/g, "")
    .replace(/\\/g, "/")
    .trim()
    .split("\n")
    .sort()
    .join("\n");
}

function normalizeSourceCode(source: string): string {
  return source.replace(/\r/g, "");
}

test(async function testPrettierCheckAndFormatFiles(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const files = [
    join(tempDir, "0.ts"),
    join(tempDir, "1.js"),
    join(tempDir, "2.ts")
  ];

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEquals(code, 1);
  assertEquals(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, "--write", ...files]);
  assertEquals(code, 0);
  assertEquals(
    normalizeOutput(stdout),
    normalizeOutput(`Formatting ${tempDir}/0.ts
Formatting ${tempDir}/1.js`)
  );

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEquals(code, 0);
  assertEquals(normalizeOutput(stdout), "Every file is formatted");

  emptyDir(tempDir);
});

test(async function testPrettierCheckAndFormatDirs(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const dirs = [join(tempDir, "foo"), join(tempDir, "bar")];

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEquals(code, 1);
  assertEquals(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, "--write", ...dirs]);
  assertEquals(code, 0);
  assertEquals(
    normalizeOutput(stdout),
    normalizeOutput(`Formatting ${tempDir}/bar/0.ts
Formatting ${tempDir}/bar/1.js
Formatting ${tempDir}/foo/0.ts
Formatting ${tempDir}/foo/1.js`)
  );

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEquals(code, 0);
  assertEquals(normalizeOutput(stdout), "Every file is formatted");

  emptyDir(tempDir);
});

test(async function testPrettierOptions(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const file0 = join(tempDir, "opts", "0.ts");
  const file1 = join(tempDir, "opts", "1.ts");
  const file2 = join(tempDir, "opts", "2.ts");
  const file3 = join(tempDir, "opts", "3.md");

  const getSourceCode = async (f: string): Promise<string> =>
    decoder.decode(await Deno.readFile(f));

  await run([...cmd, "--no-semi", "--write", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0)
console.log([function foo() {}, function baz() {}, a => {}])
`
  );

  await run([
    ...cmd,
    "--print-width",
    "30",
    "--tab-width",
    "4",
    "--write",
    file0
  ]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([
    function foo() {},
    function baz() {},
    a => {}
]);
`
  );

  await run([...cmd, "--print-width", "30", "--use-tabs", "--write", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([
	function foo() {},
	function baz() {},
	a => {}
]);
`
  );

  await run([...cmd, "--single-quote", "--write", file1]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file1)),
    `console.log('1');
`
  );

  await run([
    ...cmd,
    "--print-width",
    "30",
    "--trailing-comma",
    "all",
    "--write",
    file0
  ]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([
  function foo() {},
  function baz() {},
  a => {},
]);
`
  );

  await run([...cmd, "--no-bracket-spacing", "--write", file2]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file2)),
    `console.log({a: 1});
`
  );

  await run([...cmd, "--arrow-parens", "always", "--write", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([function foo() {}, function baz() {}, (a) => {}]);
`
  );

  await run([...cmd, "--prose-wrap", "always", "--write", file3]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file3)),
    `Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
`
  );

  await run([...cmd, "--end-of-line", "crlf", "--write", file2]);
  assertEquals(await getSourceCode(file2), "console.log({ a: 1 });\r\n");

  emptyDir(tempDir);
});

test(async function testPrettierPrintToStdout(): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  await copy(testdata, tempDir, { overwrite: true });

  const file0 = join(tempDir, "0.ts");
  const file1 = join(tempDir, "formatted.ts");

  const getSourceCode = async (f: string): Promise<string> =>
    decoder.decode(await Deno.readFile(f));

  const { stdout } = await run([...cmd, file0]);
  // The source file will not change without `--write` flags.
  assertEquals(await getSourceCode(file0), "console.log (0)" + EOL);
  // The output should be formatted code.
  assertEquals(stdout, "console.log(0);" + EOL);

  const { stdout: formattedCode } = await run([...cmd, file1]);
  // The source file will not change without `--write` flags.
  assertEquals(await getSourceCode(file1), "console.log(0);" + EOL);
  // The output will be formatted code even it is the same as the source file's content.
  assertEquals(formattedCode, "console.log(0);" + EOL);

  emptyDir(tempDir);
});
