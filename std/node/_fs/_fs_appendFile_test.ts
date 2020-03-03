const { test } = Deno;
import { assertEquals, assert, assertThrows, assertThrowsAsync } from "../../testing/asserts.ts";
import { appendFile, appendFileSync } from "./_fs_appendFile.ts";

test(async function noCallbackFnResultsInError() {
  assertThrows(() => appendFile("some/path", "some data", "utf8"), Error, 'No callback function supplied');
});

