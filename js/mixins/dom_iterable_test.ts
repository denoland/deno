// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "../test_util.ts";
import { DomIterableMixin } from "deno";

function setup() {
  const dataSymbol = Symbol("data symbol");
  class Base {
    private [dataSymbol] = new Map<string, number>();

    constructor(
      data: Array<[string, number]> | IterableIterator<[string, number]>
    ) {
      for (const [key, value] of data) {
        this[dataSymbol].set(key, value);
      }
    }
  }

  return {
    Base,
    DomIterable: DomIterableMixin<string, number, typeof Base>(Base, dataSymbol)
  };
}

test(function testDomIterable() {
  // tslint:disable-next-line:variable-name
  const { DomIterable, Base } = setup();

  const fixture: Array<[string, number]> = [["foo", 1], ["bar", 2]];

  const domIterable = new DomIterable(fixture);

  assertEqual(Array.from(domIterable.entries()), fixture);
  assertEqual(Array.from(domIterable.values()), [1, 2]);
  assertEqual(Array.from(domIterable.keys()), ["foo", "bar"]);

  let result: Array<[string, number]> = [];
  for (const [key, value] of domIterable) {
    assert(key != null);
    assert(value != null);
    result.push([key, value]);
  }
  assertEqual(fixture, result);

  result = [];
  const scope = {};
  function callback(value, key, parent) {
    assertEqual(parent, domIterable);
    assert(key != null);
    assert(value != null);
    assert(this === scope);
    result.push([key, value]);
  }
  domIterable.forEach(callback, scope);
  assertEqual(fixture, result);

  assertEqual(DomIterable.name, Base.name);
});

test(function testDomIterableScope() {
  // tslint:disable-next-line:variable-name
  const { DomIterable } = setup();

  const domIterable = new DomIterable([["foo", 1]]);

  // tslint:disable-next-line:no-any
  function checkScope(thisArg: any, expected: any) {
    function callback() {
      assertEqual(this, expected);
    }
    domIterable.forEach(callback, thisArg);
  }

  checkScope(0, Object(0));
  checkScope("", Object(""));
  checkScope(null, window);
  checkScope(undefined, window);
});
