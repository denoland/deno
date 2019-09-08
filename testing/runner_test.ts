// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { getMatchingUrls } from "./runner.ts";
import { join } from "../fs/path/mod.ts";

/**
 * IMPORTANT: This file assumes it is run from root of repository.
 */
const cwd = Deno.cwd();
const TEST_ROOT_PATH = join(cwd, "fmt");

test(async function getMatchingUrlsRemote(): Promise<void> {
  const matches = [
    "https://deno.land/std/fmt/colors_test.ts",
    "http://deno.land/std/fmt/printf_test.ts"
  ];

  const urls = await getMatchingUrls(matches, [], TEST_ROOT_PATH);
  assertEquals(urls, matches);
});

test(async function getMatchingUrlsLocal(): Promise<void> {
  const urls = await getMatchingUrls(
    ["fmt/*_test.ts"],
    ["colors*"],
    TEST_ROOT_PATH
  );
  assertEquals(urls.length, 1);
});
