// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals } from "./test_util.ts";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
function setup() {
  const dataSymbol = Symbol("data symbol");
  class Base {
    [dataSymbol] = new Map<string, number>();

    constructor(
      data: Array<[string, number]> | IterableIterator<[string, number]>,
    ) {
      for (const [key, value] of data) {
        this[dataSymbol].set(key, value);
      }
    }
  }

  return {
    Base,
    // This is using an internal API we don't want published as types, so having
    // to cast to any to "trick" TypeScript
    // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
    DomIterable: Deno[Deno.internal].DomIterableMixin(Base, dataSymbol),
  };
}

Deno.test("testDomIterable", function (): void {
  const { DomIterable, Base } = setup();

  const fixture: Array<[string, number]> = [
    ["foo", 1],
    ["bar", 2],
  ];

  const domIterable = new DomIterable(fixture);

  assertEquals(Array.from(domIterable.entries()), fixture);
  assertEquals(Array.from(domIterable.values()), [1, 2]);
  assertEquals(Array.from(domIterable.keys()), ["foo", "bar"]);

  let result: Array<[string, number]> = [];
  for (const [key, value] of domIterable) {
    assert(key != null);
    assert(value != null);
    result.push([key, value]);
  }
  assertEquals(fixture, result);

  result = [];
  const scope = {};
  function callback(
    this: typeof scope,
    value: number,
    key: string,
    parent: typeof domIterable,
  ): void {
    assertEquals(parent, domIterable);
    assert(key != null);
    assert(value != null);
    assert(this === scope);
    result.push([key, value]);
  }
  domIterable.forEach(callback, scope);
  assertEquals(fixture, result);

  assertEquals(DomIterable.name, Base.name);
});

Deno.test("testDomIterableScope", function (): void {
  const { DomIterable } = setup();

  const domIterable = new DomIterable([["foo", 1]]);

  // deno-lint-ignore no-explicit-any
  function checkScope(thisArg: any, expected: any): void {
    function callback(this: typeof thisArg): void {
      assertEquals(this, expected);
    }
    domIterable.forEach(callback, thisArg);
  }

  checkScope(0, Object(0));
  checkScope("", Object(""));
  checkScope(null, window);
  checkScope(undefined, window);
});
