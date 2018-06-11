// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
// This test is executed as part of integration_test.go
// But it can also be run manually:
//  ./deno tests.ts
// There must also be a static file http server running on localhost:4545
// serving the deno project directory. Try this:
//   http-server -p 4545 --cors .
import { test, assert, assertEqual } from "./testing/testing.ts";
import { readFileSync, writeFileSync } from "deno";

test(async function tests_test() {
  assert(true);
});

test(async function tests_fetch() {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  assertEqual(json.name, "deno");
});

test(function tests_console_assert() {
  console.assert(true);

  let hasThrown = false;
  try {
    console.assert(false);
  } catch {
    hasThrown = true;
  }
  assertEqual(hasThrown, true);
});

test(async function tests_readFileSync() {
  const data = readFileSync("package.json");
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

test(function tests_console_assert() {
  console.assert(true);

  let hasThrown = false;
  try {
    console.assert(false);
  } catch {
    hasThrown = true;
  }
  assertEqual(hasThrown, true);
});

test(function tests_console_stringify_circular() {
  class Base {
    a = 1;
    m1() {}
  }

  class Extended extends Base {
    b = 2;
    m2() {}
  }

  // tslint:disable-next-line:no-any
  const nestedObj: any = {
    num: 1,
    bool: true,
    str: "a",
    method() {},
    un: undefined,
    nu: null,
    arrowFunc: () => {},
    extendedClass: new Extended(),
    nFunc: new Function(),
    extendedCstr: Extended,
  };

  const circularObj = {
    num: 2,
    bool: false,
    str: "b",
    method() {},
    un: undefined,
    nu: null,
    nested: nestedObj,
    emptyObj: {},
    arr: [1, "s", false, null, nestedObj],
    baseClass: new Base(),
  };

  nestedObj.o = circularObj;

  try {
    console.log(1);
    console.log("s");
    console.log(false);
    console.log(Symbol(1));
    console.log(null);
    console.log(undefined);
    console.log(new Extended());
    console.log(function f() {});
    console.log(nestedObj);
    console.log(JSON);
    console.log(console);
  } catch {
    throw new Error(
      "Expected no crash on circular object"
    );
  }
});
