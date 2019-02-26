// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "../testing/mod.ts";
import { xrun, executableSuffix } from "./util.ts";
const { readAll } = Deno;

const decoder = new TextDecoder();

async function run(args: string[]) {
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

async function clearTestdataChanges() {
  await xrun({ args: ["git", "checkout", testdata] }).status();
}

test(async function testPrettierCheckAndFormatFiles() {
  await clearTestdataChanges();

  const files = [`${testdata}/0.ts`, `${testdata}/1.js`];

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEqual(code, 1);
  assertEqual(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, ...files]);
  assertEqual(code, 0);
  assertEqual(
    normalizeOutput(stdout),
    `Formatting prettier/testdata/0.ts
Formatting prettier/testdata/1.js`
  );

  var { code, stdout } = await run([...cmd, "--check", ...files]);
  assertEqual(code, 0);
  assertEqual(normalizeOutput(stdout), "Every file is formatted");

  await clearTestdataChanges();
});

test(async function testPrettierCheckAndFormatDirs() {
  await clearTestdataChanges();

  const dirs = [`${testdata}/foo`, `${testdata}/bar`];

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEqual(code, 1);
  assertEqual(normalizeOutput(stdout), "Some files are not formatted");

  var { code, stdout } = await run([...cmd, ...dirs]);
  assertEqual(code, 0);
  assertEqual(
    normalizeOutput(stdout),
    `Formatting prettier/testdata/bar/0.ts
Formatting prettier/testdata/bar/1.js
Formatting prettier/testdata/foo/0.ts
Formatting prettier/testdata/foo/1.js`
  );

  var { code, stdout } = await run([...cmd, "--check", ...dirs]);
  assertEqual(code, 0);
  assertEqual(normalizeOutput(stdout), "Every file is formatted");

  await clearTestdataChanges();
});
