// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { xrun, executableSuffix } from "./util.ts";
import { assertEq } from "../testing/asserts.ts";
const { readAll } = Deno;

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
  `deno${executableSuffix}`,
  "--allow-run",
  "--allow-write",
  "--allow-read",
  "prettier/main.ts"
];
const testdata = "prettier/testdata";

function normalizeOutput(output: string): string {
  return output
    .replace(/\r/g, "")
    .replace(/\\/g, "/")
    .trim()
    .split("\n")
    .sort()
    .join("\n");
}

async function clearTestdataChanges(): Promise<void> {
  await xrun({ args: ["git", "checkout", testdata] }).status();
}

test(async function testPrettierCheckAndFormatFiles() {
  await clearTestdataChanges();

  const files = [`${testdata}/0.ts`, `${testdata}/1.js`];

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEq(code, 1);
  assertEq(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, ...files]);
  assertEq(code, 0);
  assertEq(
    normalizeOutput(stdout),
    `Formatting prettier/testdata/0.ts
Formatting prettier/testdata/1.js`
  );

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEq(code, 0);
  assertEq(normalizeOutput(stdout), "Every file is formatted");

  await clearTestdataChanges();
});

test(async function testPrettierCheckAndFormatDirs() {
  await clearTestdataChanges();

  const dirs = [`${testdata}/foo`, `${testdata}/bar`];

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEq(code, 1);
  assertEq(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, ...dirs]);
  assertEq(code, 0);
  assertEq(
    normalizeOutput(stdout),
    `Formatting prettier/testdata/bar/0.ts
Formatting prettier/testdata/bar/1.js
Formatting prettier/testdata/foo/0.ts
Formatting prettier/testdata/foo/1.js`
  );

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEq(code, 0);
  assertEq(normalizeOutput(stdout), "Every file is formatted");

  await clearTestdataChanges();
});
