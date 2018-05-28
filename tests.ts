// This test is executed as part of integration_test.go
// But it can also be run manually:
//  ./deno tests.ts
import { test, assert, assertEqual } from "./deno_testing/testing.ts";
import { readFileSync } from "deno";

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

test(async function tests_fetch() {
  const response = await fetch('http://localhost:4545/package.json');
  const json = await response.json();
  assertEqual(json.name, "deno");
});
