// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { join } from "../fs/path.ts";
import { assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import { xrun, executableSuffix } from "./util.ts";
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

async function clearTestdataChanges(): Promise<void> {
  await xrun({ args: ["git", "checkout", testdata] }).status();
}

test(async function testPrettierCheckAndFormatFiles() {
  await clearTestdataChanges();

  const files = [join(testdata, "0.ts"), join(testdata, "1.js")];

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
