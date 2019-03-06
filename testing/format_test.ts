// This file is ported from pretty-format@24.0.0
/**
 * Copyright (c) Facebook, Inc. and its affiliates. All Rights Reserved.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import { test } from "./mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { format } from "./format.ts";

// eslint-disable-next-line @typescript-eslint/no-unused-vars,@typescript-eslint/no-explicit-any
function returnArguments(...args: any[]): IArguments {
  return arguments;
}

function MyObject(value: unknown): void {
  // @ts-ignore
  this.name = value;
}

class MyArray<T> extends Array<T> {}

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
const createVal = () => [
  {
    id: "8658c1d0-9eda-4a90-95e1-8001e8eb6036",
    text: "Add alternative serialize API for pretty-format plugins",
    type: "ADD_TODO"
  },
  {
    id: "8658c1d0-9eda-4a90-95e1-8001e8eb6036",
    type: "TOGGLE_TODO"
  }
];

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
const createExpected = () =>
  [
    "Array [",
    "  Object {",
    '    "id": "8658c1d0-9eda-4a90-95e1-8001e8eb6036",',
    '    "text": "Add alternative serialize API for pretty-format plugins",',
    '    "type": "ADD_TODO",',
    "  },",
    "  Object {",
    '    "id": "8658c1d0-9eda-4a90-95e1-8001e8eb6036",',
    '    "type": "TOGGLE_TODO",',
    "  },",
    "]"
  ].join("\n");

test({
  name: "prints empty arguments",
  fn() {
    const val = returnArguments();
    assertEq(format(val), "Arguments []");
  }
});

test({
  name: "prints an empty array",
  fn() {
    const val: unknown[] = [];
    assertEq(format(val), "Array []");
  }
});

test({
  name: "prints an array with items",
  fn() {
    const val = [1, 2, 3];
    assertEq(format(val), "Array [\n  1,\n  2,\n  3,\n]");
  }
});

test({
  name: "prints a empty typed array",
  fn() {
    const val = new Uint32Array(0);
    assertEq(format(val), "Uint32Array []");
  }
});

test({
  name: "prints a typed array with items",
  fn() {
    const val = new Uint32Array(3);
    assertEq(format(val), "Uint32Array [\n  0,\n  0,\n  0,\n]");
  }
});

test({
  name: "prints an array buffer",
  fn() {
    const val = new ArrayBuffer(3);
    assertEq(format(val), "ArrayBuffer []");
  }
});

test({
  name: "prints a nested array",
  fn() {
    const val = [[1, 2, 3]];
    assertEq(
      format(val),
      "Array [\n  Array [\n    1,\n    2,\n    3,\n  ],\n]"
    );
  }
});

test({
  name: "prints true",
  fn() {
    const val = true;
    assertEq(format(val), "true");
  }
});

test({
  name: "prints false",
  fn() {
    const val = false;
    assertEq(format(val), "false");
  }
});

test({
  name: "prints an error",
  fn() {
    const val = new Error();
    assertEq(format(val), "[Error]");
  }
});

test({
  name: "prints a typed error with a message",
  fn() {
    const val = new TypeError("message");
    assertEq(format(val), "[TypeError: message]");
  }
});

test({
  name: "prints a function constructor",
  fn() {
    // tslint:disable-next-line:function-constructor
    const val = new Function();
    assertEq(format(val), "[Function anonymous]");
  }
});

test({
  name: "prints an anonymous callback function",
  fn() {
    let val;
    function f(cb: () => void): void {
      val = cb;
    }
    // tslint:disable-next-line:no-empty
    f(() => {});
    assertEq(format(val), "[Function anonymous]");
  }
});

test({
  name: "prints an anonymous assigned function",
  fn() {
    // tslint:disable-next-line:no-empty
    const val = (): void => {};
    const formatted = format(val);
    assertEq(
      formatted === "[Function anonymous]" || formatted === "[Function val]",
      true
    );
  }
});

test({
  name: "prints a named function",
  fn() {
    // tslint:disable-next-line:no-empty
    const val = function named(): void {};
    assertEq(format(val), "[Function named]");
  }
});

test({
  name: "prints a named generator function",
  fn() {
    const val = function* generate(): IterableIterator<number> {
      yield 1;
      yield 2;
      yield 3;
    };
    assertEq(format(val), "[Function generate]");
  }
});

test({
  name: "can customize function names",
  fn() {
    // tslint:disable-next-line:no-empty
    const val = function named(): void {};
    assertEq(
      format(val, {
        printFunctionName: false
      }),
      "[Function]"
    );
  }
});

test({
  name: "prints Infinity",
  fn() {
    const val = Infinity;
    assertEq(format(val), "Infinity");
  }
});

test({
  name: "prints -Infinity",
  fn() {
    const val = -Infinity;
    assertEq(format(val), "-Infinity");
  }
});

test({
  name: "prints an empty map",
  fn() {
    const val = new Map();
    assertEq(format(val), "Map {}");
  }
});

test({
  name: "prints a map with values",
  fn() {
    const val = new Map();
    val.set("prop1", "value1");
    val.set("prop2", "value2");
    assertEq(
      format(val),
      'Map {\n  "prop1" => "value1",\n  "prop2" => "value2",\n}'
    );
  }
});

test({
  name: "prints a map with non-string keys",
  fn() {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const val = new Map<any, any>([
      [false, "boolean"],
      ["false", "string"],
      [0, "number"],
      ["0", "string"],
      [null, "null"],
      ["null", "string"],
      [undefined, "undefined"],
      ["undefined", "string"],
      [Symbol("description"), "symbol"],
      ["Symbol(description)", "string"],
      [["array", "key"], "array"],
      [{ key: "value" }, "object"]
    ]);
    const expected = [
      "Map {",
      '  false => "boolean",',
      '  "false" => "string",',
      '  0 => "number",',
      '  "0" => "string",',
      '  null => "null",',
      '  "null" => "string",',
      '  undefined => "undefined",',
      '  "undefined" => "string",',
      '  Symbol(description) => "symbol",',
      '  "Symbol(description)" => "string",',
      "  Array [",
      '    "array",',
      '    "key",',
      '  ] => "array",',
      "  Object {",
      '    "key": "value",',
      '  } => "object",',
      "}"
    ].join("\n");
    assertEq(format(val), expected);
  }
});

test({
  name: "prints NaN",
  fn() {
    const val = NaN;
    assertEq(format(val), "NaN");
  }
});

test({
  name: "prints null",
  fn() {
    const val = null;
    assertEq(format(val), "null");
  }
});

test({
  name: "prints a positive number",
  fn() {
    const val = 123;
    assertEq(format(val), "123");
  }
});

test({
  name: "prints a negative number",
  fn() {
    const val = -123;
    assertEq(format(val), "-123");
  }
});

test({
  name: "prints zero",
  fn() {
    const val = 0;
    assertEq(format(val), "0");
  }
});

test({
  name: "prints negative zero",
  fn() {
    const val = -0;
    assertEq(format(val), "-0");
  }
});

test({
  name: "prints a date",
  fn() {
    const val = new Date(10e11);
    assertEq(format(val), "2001-09-09T01:46:40.000Z");
  }
});

test({
  name: "prints an invalid date",
  fn() {
    const val = new Date(Infinity);
    assertEq(format(val), "Date { NaN }");
  }
});

test({
  name: "prints an empty object",
  fn() {
    const val = {};
    assertEq(format(val), "Object {}");
  }
});

test({
  name: "prints an object with properties",
  fn() {
    const val = { prop1: "value1", prop2: "value2" };
    assertEq(
      format(val),
      'Object {\n  "prop1": "value1",\n  "prop2": "value2",\n}'
    );
  }
});

test({
  name: "prints an object with properties and symbols",
  fn() {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const val: any = {};
    val[Symbol("symbol1")] = "value2";
    val[Symbol("symbol2")] = "value3";
    val.prop = "value1";
    assertEq(
      format(val),
      'Object {\n  "prop": "value1",\n  Symbol(symbol1): "value2",\n  Symbol(symbol2): "value3",\n}'
    );
  }
});

test({
  name:
    "prints an object without non-enumerable properties which have string key",
  fn() {
    const val = {
      enumerable: true
    };
    const key = "non-enumerable";
    Object.defineProperty(val, key, {
      enumerable: false,
      value: false
    });
    assertEq(format(val), 'Object {\n  "enumerable": true,\n}');
  }
});

test({
  name:
    "prints an object without non-enumerable properties which have symbol key",
  fn() {
    const val = {
      enumerable: true
    };
    const key = Symbol("non-enumerable");
    Object.defineProperty(val, key, {
      enumerable: false,
      value: false
    });
    assertEq(format(val), 'Object {\n  "enumerable": true,\n}');
  }
});

test({
  name: "prints an object with sorted properties",
  fn() {
    const val = { b: 1, a: 2 };
    assertEq(format(val), 'Object {\n  "a": 2,\n  "b": 1,\n}');
  }
});

test({
  name: "prints regular expressions from constructors",
  fn() {
    const val = new RegExp("regexp");
    assertEq(format(val), "/regexp/");
  }
});

test({
  name: "prints regular expressions from literals",
  fn() {
    const val = /regexp/gi;
    assertEq(format(val), "/regexp/gi");
  }
});

test({
  name: "prints regular expressions {escapeRegex: false}",
  fn() {
    const val = /regexp\d/gi;
    assertEq(format(val), "/regexp\\d/gi");
  }
});

test({
  name: "prints regular expressions {escapeRegex: true}",
  fn() {
    const val = /regexp\d/gi;
    assertEq(format(val, { escapeRegex: true }), "/regexp\\\\d/gi");
  }
});

test({
  name: "escapes regular expressions nested inside object",
  fn() {
    const obj = { test: /regexp\d/gi };
    assertEq(
      format(obj, { escapeRegex: true }),
      'Object {\n  "test": /regexp\\\\d/gi,\n}'
    );
  }
});

test({
  name: "prints an empty set",
  fn() {
    const val = new Set();
    assertEq(format(val), "Set {}");
  }
});

test({
  name: "prints a set with values",
  fn() {
    const val = new Set();
    val.add("value1");
    val.add("value2");
    assertEq(format(val), 'Set {\n  "value1",\n  "value2",\n}');
  }
});

test({
  name: "prints a string",
  fn() {
    const val = "string";
    assertEq(format(val), '"string"');
  }
});

test({
  name: "prints and escape a string",
  fn() {
    const val = "\"'\\";
    assertEq(format(val), '"\\"\'\\\\"');
  }
});

test({
  name: "doesn't escape string with {excapeString: false}",
  fn() {
    const val = "\"'\\n";
    assertEq(format(val, { escapeString: false }), '""\'\\n"');
  }
});

test({
  name: "prints a string with escapes",
  fn() {
    assertEq(format('"-"'), '"\\"-\\""');
    assertEq(format("\\ \\\\"), '"\\\\ \\\\\\\\"');
  }
});

test({
  name: "prints a multiline string",
  fn() {
    const val = ["line 1", "line 2", "line 3"].join("\n");
    assertEq(format(val), '"' + val + '"');
  }
});

test({
  name: "prints a multiline string as value of object property",
  fn() {
    const polyline = {
      props: {
        id: "J",
        points: ["0.5,0.460", "0.5,0.875", "0.25,0.875"].join("\n")
      },
      type: "polyline"
    };
    const val = {
      props: {
        children: polyline
      },
      type: "svg"
    };
    assertEq(
      format(val),
      [
        "Object {",
        '  "props": Object {',
        '    "children": Object {',
        '      "props": Object {',
        '        "id": "J",',
        '        "points": "0.5,0.460',
        "0.5,0.875",
        '0.25,0.875",',
        "      },",
        '      "type": "polyline",',
        "    },",
        "  },",
        '  "type": "svg",',
        "}"
      ].join("\n")
    );
  }
});

test({
  name: "prints a symbol",
  fn() {
    const val = Symbol("symbol");
    assertEq(format(val), "Symbol(symbol)");
  }
});

test({
  name: "prints undefined",
  fn() {
    const val = undefined;
    assertEq(format(val), "undefined");
  }
});

test({
  name: "prints a WeakMap",
  fn() {
    const val = new WeakMap();
    assertEq(format(val), "WeakMap {}");
  }
});

test({
  name: "prints a WeakSet",
  fn() {
    const val = new WeakSet();
    assertEq(format(val), "WeakSet {}");
  }
});

test({
  name: "prints deeply nested objects",
  fn() {
    const val = { prop: { prop: { prop: "value" } } };
    assertEq(
      format(val),
      'Object {\n  "prop": Object {\n    "prop": Object {\n      "prop": "value",\n    },\n  },\n}'
    );
  }
});

test({
  name: "prints circular references",
  fn() {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const val: any = {};
    val.prop = val;
    assertEq(format(val), 'Object {\n  "prop": [Circular],\n}');
  }
});

test({
  name: "prints parallel references",
  fn() {
    const inner = {};
    const val = { prop1: inner, prop2: inner };
    assertEq(
      format(val),
      'Object {\n  "prop1": Object {},\n  "prop2": Object {},\n}'
    );
  }
});

test({
  name: "default implicit: 2 spaces",
  fn() {
    assertEq(format(createVal()), createExpected());
  }
});

test({
  name: "default explicit: 2 spaces",
  fn() {
    assertEq(format(createVal(), { indent: 2 }), createExpected());
  }
});

// Tests assume that no strings in val contain multiple adjacent spaces!
test({
  name: "non-default: 0 spaces",
  fn() {
    const indent = 0;
    assertEq(
      format(createVal(), { indent }),
      createExpected().replace(/ {2}/g, " ".repeat(indent))
    );
  }
});

test({
  name: "non-default: 4 spaces",
  fn() {
    const indent = 4;
    assertEq(
      format(createVal(), { indent }),
      createExpected().replace(/ {2}/g, " ".repeat(indent))
    );
  }
});

test({
  name: "can customize the max depth",
  fn() {
    const v = [
      {
        "arguments empty": returnArguments(),
        "arguments non-empty": returnArguments("arg"),
        "array literal empty": [],
        "array literal non-empty": ["item"],
        "extended array empty": new MyArray(),
        "map empty": new Map(),
        "map non-empty": new Map([["name", "value"]]),
        "object literal empty": {},
        "object literal non-empty": { name: "value" },
        // @ts-ignore
        "object with constructor": new MyObject("value"),
        "object without constructor": Object.create(null),
        "set empty": new Set(),
        "set non-empty": new Set(["value"])
      }
    ];
    assertEq(
      format(v, { maxDepth: 2 }),
      [
        "Array [",
        "  Object {",
        '    "arguments empty": [Arguments],',
        '    "arguments non-empty": [Arguments],',
        '    "array literal empty": [Array],',
        '    "array literal non-empty": [Array],',
        '    "extended array empty": [MyArray],',
        '    "map empty": [Map],',
        '    "map non-empty": [Map],',
        '    "object literal empty": [Object],',
        '    "object literal non-empty": [Object],',
        '    "object with constructor": [MyObject],',
        '    "object without constructor": [Object],',
        '    "set empty": [Set],',
        '    "set non-empty": [Set],',
        "  },",
        "]"
      ].join("\n")
    );
  }
});

test({
  name: "prints objects with no constructor",
  fn() {
    assertEq(format(Object.create(null)), "Object {}");
  }
});

test({
  name: "prints identity-obj-proxy with string constructor",
  fn() {
    const obj = Object.create(null);
    obj.constructor = "constructor";
    const expected = [
      "Object {", // Object instead of undefined
      '  "constructor": "constructor",',
      "}"
    ].join("\n");
    assertEq(format(obj), expected);
  }
});

test({
  name: "calls toJSON and prints its return value",
  fn() {
    assertEq(
      format({
        toJSON: () => ({ value: false }),
        value: true
      }),
      'Object {\n  "value": false,\n}'
    );
  }
});

test({
  name: "calls toJSON and prints an internal representation.",
  fn() {
    assertEq(
      format({
        toJSON: () => "[Internal Object]",
        value: true
      }),
      '"[Internal Object]"'
    );
  }
});

test({
  name: "calls toJSON only on functions",
  fn() {
    assertEq(
      format({
        toJSON: false,
        value: true
      }),
      'Object {\n  "toJSON": false,\n  "value": true,\n}'
    );
  }
});

test({
  name: "does not call toJSON recursively",
  fn() {
    assertEq(
      format({
        toJSON: () => ({ toJSON: () => ({ value: true }) }),
        value: false
      }),
      'Object {\n  "toJSON": [Function toJSON],\n}'
    );
  }
});

test({
  name: "calls toJSON on Sets",
  fn() {
    const set = new Set([1]);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (set as any).toJSON = () => "map";
    assertEq(format(set), '"map"');
  }
});
