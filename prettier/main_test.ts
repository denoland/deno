// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { join } from "../fs/path.ts";
import { assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import { xrun } from "./util.ts";
const { readAll, execPath } = Deno;

const decoder = new TextDecoder();

async function run(
  args: string[]
): Promise<{ stdout: string; code: number | undefined }> {
  const p = xrun({ args, stdout: "piped" });

  const stdout = decoder.decode(await readAll(p.stdout));
  const { code } = await p.status();

  return { stdout, code };
}

const cmd = [
  execPath,
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

async function clearTestdataChanges(): Promise<void> {
  await xrun({ args: ["git", "checkout", testdata] }).status();
}

test(async function testPrettierCheckAndFormatFiles() {
  await clearTestdataChanges();

  const files = [
    join(testdata, "0.ts"),
    join(testdata, "1.js"),
    join(testdata, "2.ts")
  ];

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEquals(code, 1);
  assertEquals(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, ...files]);
  assertEquals(code, 0);
  assertEquals(
    normalizeOutput(stdout),
    `Formatting ./prettier/testdata/0.ts
Formatting ./prettier/testdata/1.js`
  );

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEquals(code, 0);
  assertEquals(normalizeOutput(stdout), "Every file is formatted");

  await clearTestdataChanges();
});

test(async function testPrettierCheckAndFormatDirs() {
  await clearTestdataChanges();

  const dirs = [join(testdata, "foo"), join(testdata, "bar")];

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEquals(code, 1);
  assertEquals(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, ...dirs]);
  assertEquals(code, 0);
  assertEquals(
    normalizeOutput(stdout),
    `Formatting ./prettier/testdata/bar/0.ts
Formatting ./prettier/testdata/bar/1.js
Formatting ./prettier/testdata/foo/0.ts
Formatting ./prettier/testdata/foo/1.js`
  );

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEquals(code, 0);
  assertEquals(normalizeOutput(stdout), "Every file is formatted");

  await clearTestdataChanges();
});

test(async function testPrettierOptions() {
  await clearTestdataChanges();

  const file0 = join(testdata, "opts", "0.ts");
  const file1 = join(testdata, "opts", "1.ts");
  const file2 = join(testdata, "opts", "2.ts");
  const file3 = join(testdata, "opts", "3.md");

  const getSourceCode = async (f: string): Promise<string> =>
    decoder.decode(await Deno.readFile(f));

  await run([...cmd, "--no-semi", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0)
console.log([function foo() {}, function baz() {}, a => {}])
`
  );

  await run([...cmd, "--print-width", "30", "--tab-width", "4", file0]);
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

  await run([...cmd, "--print-width", "30", "--use-tabs", file0]);
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

  await run([...cmd, "--single-quote", file1]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file1)),
    `console.log('1');
`
  );

  await run([...cmd, "--print-width", "30", "--trailing-comma", "all", file0]);
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

  await run([...cmd, "--no-bracket-spacing", file2]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file2)),
    `console.log({a: 1});
`
  );

  await run([...cmd, "--arrow-parens", "always", file0]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file0)),
    `console.log(0);
console.log([function foo() {}, function baz() {}, (a) => {}]);
`
  );

  await run([...cmd, "--prose-wrap", "always", file3]);
  assertEquals(
    normalizeSourceCode(await getSourceCode(file3)),
    `Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor
incididunt ut labore et dolore magna aliqua.
`
  );

  await run([...cmd, "--end-of-line", "crlf", file2]);
  assertEquals(await getSourceCode(file2), "console.log({ a: 1 });\r\n");

  await clearTestdataChanges();
});
