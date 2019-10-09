// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "./mod.ts";
import { findTestModules } from "./runner.ts";
import { isWindows } from "../fs/path/constants.ts";
import { assertEquals } from "../testing/asserts.ts";
const { cwd } = Deno;

function urlToFilePath(url: URL): string {
  // Since `new URL('file:///C:/a').pathname` is `/C:/a`, remove leading slash.
  return url.pathname.slice(url.protocol == "file:" && isWindows ? 1 : 0);
}

async function findTestModulesArray(
  include: string[],
  exclude: string[],
  root: string = cwd()
): Promise<string[]> {
  const result = [];
  for await (const testModule of findTestModules(include, exclude, root)) {
    result.push(testModule);
  }
  return result;
}

const TEST_DATA_URL = new URL("testdata", import.meta.url);
const TEST_DATA_PATH = urlToFilePath(TEST_DATA_URL);

test(async function findTestModulesDir1(): Promise<void> {
  const urls = await findTestModulesArray(["."], [], TEST_DATA_PATH);
  assertEquals(urls.sort(), [
    `${TEST_DATA_URL}/bar_test.js`,
    `${TEST_DATA_URL}/foo_test.ts`,
    `${TEST_DATA_URL}/subdir/bar_test.js`,
    `${TEST_DATA_URL}/subdir/foo_test.ts`,
    `${TEST_DATA_URL}/subdir/test.js`,
    `${TEST_DATA_URL}/subdir/test.ts`,
    `${TEST_DATA_URL}/test.js`,
    `${TEST_DATA_URL}/test.ts`
  ]);
});

test(async function findTestModulesDir2(): Promise<void> {
  const urls = await findTestModulesArray(["subdir"], [], TEST_DATA_PATH);
  assertEquals(urls.sort(), [
    `${TEST_DATA_URL}/subdir/bar_test.js`,
    `${TEST_DATA_URL}/subdir/foo_test.ts`,
    `${TEST_DATA_URL}/subdir/test.js`,
    `${TEST_DATA_URL}/subdir/test.ts`
  ]);
});

test(async function findTestModulesGlob(): Promise<void> {
  const urls = await findTestModulesArray(
    ["**/*_test.{js,ts}"],
    [],
    TEST_DATA_PATH
  );
  assertEquals(urls.sort(), [
    `${TEST_DATA_URL}/bar_test.js`,
    `${TEST_DATA_URL}/foo_test.ts`,
    `${TEST_DATA_URL}/subdir/bar_test.js`,
    `${TEST_DATA_URL}/subdir/foo_test.ts`
  ]);
});

test(async function findTestModulesExcludeDir(): Promise<void> {
  const urls = await findTestModulesArray(["."], ["subdir"], TEST_DATA_PATH);
  assertEquals(urls.sort(), [
    `${TEST_DATA_URL}/bar_test.js`,
    `${TEST_DATA_URL}/foo_test.ts`,
    `${TEST_DATA_URL}/test.js`,
    `${TEST_DATA_URL}/test.ts`
  ]);
});

test(async function findTestModulesExcludeGlob(): Promise<void> {
  const urls = await findTestModulesArray(["."], ["**/foo*"], TEST_DATA_PATH);
  assertEquals(urls.sort(), [
    `${TEST_DATA_URL}/bar_test.js`,
    `${TEST_DATA_URL}/subdir/bar_test.js`,
    `${TEST_DATA_URL}/subdir/test.js`,
    `${TEST_DATA_URL}/subdir/test.ts`,
    `${TEST_DATA_URL}/test.js`,
    `${TEST_DATA_URL}/test.ts`
  ]);
});

test(async function findTestModulesRemote(): Promise<void> {
  const urls = [
    "https://example.com/colors_test.ts",
    "http://example.com/printf_test.ts"
  ];
  const matches = await findTestModulesArray(urls, []);
  assertEquals(matches, urls);
});
