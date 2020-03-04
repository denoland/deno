const { test } = Deno;
import { assertEquals, assert, assertThrows, assertThrowsAsync } from "../../testing/asserts.ts";
import { appendFile, appendFileSync } from "./_fs_appendFile.ts";

const decoder = new TextDecoder("utf-8");

test(async function noCallbackFnResultsInError() {
  assertThrows(() => appendFile("some/path", "some data", "utf8"), Error, 'No callback function supplied');
});

test(async function unsupportedEncodingResultsInError() {
  assertThrows(() => appendFile("some/path", "some data", "made-up-encoding", () => {}), Error, 'No callback function supplied');
  assertThrows(() => appendFile("some/path", "some data", {encoding: "made-up-encoding"}, () => {}), Error, 'No callback function supplied');
});

test(async function dataIsWrittenToPassedInRid() {
  const tempFile: string = await Deno.makeTempFile();
  const file: Deno.File = await Deno.open(tempFile, {create: true, write: true, read: true});
  let calledBack = false;
  await appendFile(file.rid, "hello world", () => {
    calledBack = true;
  });
  assert(calledBack);
  Deno.close(file.rid);
  const data = await Deno.readFile(tempFile);
  assertEquals(decoder.decode(data), "hello world");
});

test(async function )