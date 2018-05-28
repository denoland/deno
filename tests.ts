// This test is executed as part of integration_test.go
// But it can also be run manually:
//  ./deno tests.ts
import { test, assert, assertEqual } from "./deno_testing/testing.ts";
import { readFileSync, writeFileSync } from "deno";

test(async function tests_test() {
  assert(true);
});

test(async function tests_readFileSync() {
  let data = readFileSync("package.json");
  if (!data.byteLength) {
    throw Error(
      `Expected positive value for data.byteLength ${data.byteLength}`
    );
  }
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEqual(pkg.name, "deno");
});

test(async function tests_writeFileSync() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  // TODO need ability to get tmp dir.
  const fn = "/tmp/test.txt";
  writeFileSync("/tmp/test.txt", data, 0o666);
  const dataRead = readFileSync("/tmp/test.txt");
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

test(async function tests_fetch() {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  assertEqual(json.name, "deno");
});
