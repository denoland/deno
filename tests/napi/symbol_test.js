// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const testSymbol = loadTestLibrary();

Deno.test("napi symbol1", () => {
  const sym = testSymbol.symbolNew("test");
  assertEquals(sym.toString(), "Symbol(test)");

  const myObj = {};
  const fooSym = testSymbol.symbolNew("foo");
  const otherSym = testSymbol.symbolNew("bar");
  myObj.foo = "bar";
  myObj[fooSym] = "baz";
  myObj[otherSym] = "bing";
  assertEquals(myObj.foo, "bar");
  assertEquals(myObj[fooSym], "baz");
  assertEquals(myObj[otherSym], "bing");
});

Deno.test("napi symbol2", () => {
  const sym = testSymbol.symbolNew("test");
  assertEquals(sym.toString(), "Symbol(test)");

  const myObj = {};
  const fooSym = testSymbol.symbolNew("foo");
  myObj.foo = "bar";
  myObj[fooSym] = "baz";

  assertEquals(Object.keys(myObj), ["foo"]);
  assertEquals(Object.getOwnPropertyNames(myObj), ["foo"]);
  assertEquals(Object.getOwnPropertySymbols(myObj), [fooSym]);
});

Deno.test("napi symbol3", () => {
  assert(testSymbol.symbolNew() !== testSymbol.symbolNew());
  assert(testSymbol.symbolNew("foo") !== testSymbol.symbolNew("foo"));
  assert(testSymbol.symbolNew("foo") !== testSymbol.symbolNew("bar"));

  const foo1 = testSymbol.symbolNew("foo");
  const foo2 = testSymbol.symbolNew("foo");
  const object = {
    [foo1]: 1,
    [foo2]: 2,
  };
  assertEquals(object[foo1], 1);
  assertEquals(object[foo2], 2);
});
