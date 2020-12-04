// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrowsAsync } from "../testing/asserts.ts";

import {
  Column,
  DataItem,
  NEWLINE,
  stringify,
  StringifyError,
  StringifyOptions,
} from "./csv_stringify.ts";

type StringifyTestCaseBase = {
  columns: Column[];
  data: DataItem[];
  name: string;
  options?: StringifyOptions;
};

type StringifyTestCaseError = StringifyTestCaseBase & {
  errorMessage?: string;
  // deno-lint-ignore no-explicit-any
  throwsError: new (...args: any[]) => Error;
};

type StringifyTestCase = StringifyTestCaseBase & { expected: string };

const stringifyTestCases: (StringifyTestCase | StringifyTestCaseError)[] = [
  {
    columns: ["a"],
    data: [["foo"], ["bar"]],
    errorMessage: 'Property accessor is not of type "number"',
    name: "[CSV_stringify] Access array index using string",
    throwsError: StringifyError,
  },
  {
    columns: [0],
    data: [["foo"], ["bar"]],
    errorMessage: [
      "Separator cannot include the following strings:",
      '  - U+0022: Quotation mark (")',
      "  - U+000D U+000A: Carriage Return + Line Feed (\\r\\n)",
    ].join("\n"),
    name: "[CSV_stringify] Double quote in separator",
    options: { separator: '"' },
    throwsError: StringifyError,
  },
  {
    columns: [0],
    data: [["foo"], ["bar"]],
    errorMessage: [
      "Separator cannot include the following strings:",
      '  - U+0022: Quotation mark (")',
      "  - U+000D U+000A: Carriage Return + Line Feed (\\r\\n)",
    ].join("\n"),
    name: "[CSV_stringify] CRLF in separator",
    options: { separator: "\r\n" },
    throwsError: StringifyError,
  },
  {
    columns: [
      {
        fn: (obj) => obj.toUpperCase(),
        prop: "msg",
      },
    ],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    name: "[CSV_stringify] Transform function",
    throwsError: TypeError,
  },
  {
    columns: [],
    data: [],
    expected: NEWLINE,
    name: "[CSV_stringify] No data, no columns",
  },
  {
    columns: [],
    data: [],
    expected: ``,
    name: "[CSV_stringify] No data, no columns, no headers",
    options: { headers: false },
  },
  {
    columns: ["a"],
    data: [],
    expected: `a${NEWLINE}`,
    name: "[CSV_stringify] No data, columns",
  },
  {
    columns: ["a"],
    data: [],
    expected: ``,
    name: "[CSV_stringify] No data, columns, no headers",
    options: { headers: false },
  },
  {
    columns: [],
    data: [{ a: 1 }, { a: 2 }],
    expected: `${NEWLINE}${NEWLINE}${NEWLINE}`,
    name: "[CSV_stringify] Data, no columns",
  },
  {
    columns: [0, 1],
    data: [["foo", "bar"], ["baz", "qux"]],
    expected: `0\r1${NEWLINE}foo\rbar${NEWLINE}baz\rqux${NEWLINE}`,
    name: "[CSV_stringify] Separator: CR",
    options: { separator: "\r" },
  },
  {
    columns: [0, 1],
    data: [["foo", "bar"], ["baz", "qux"]],
    expected: `0\n1${NEWLINE}foo\nbar${NEWLINE}baz\nqux${NEWLINE}`,
    name: "[CSV_stringify] Separator: LF",
    options: { separator: "\n" },
  },
  {
    columns: [1],
    data: [{ 1: 1 }, { 1: 2 }],
    expected: `1${NEWLINE}1${NEWLINE}2${NEWLINE}`,
    name: "[CSV_stringify] Column: number accessor, Data: object",
  },
  {
    columns: [{ header: "Value", prop: "value" }],
    data: [{ value: "foo" }, { value: "bar" }],
    expected: `foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Explicit header value, no headers",
    options: { headers: false },
  },
  {
    columns: [1],
    data: [["key", "foo"], ["key", "bar"]],
    expected: `1${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Column: number accessor, Data: array",
  },
  {
    columns: [[1]],
    data: [{ 1: 1 }, { 1: 2 }],
    expected: `1${NEWLINE}1${NEWLINE}2${NEWLINE}`,
    name: "[CSV_stringify] Column: array number accessor, Data: object",
  },
  {
    columns: [[1]],
    data: [["key", "foo"], ["key", "bar"]],
    expected: `1${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Column: array number accessor, Data: array",
  },
  {
    columns: [[1, 1]],
    data: [["key", ["key", "foo"]], ["key", ["key", "bar"]]],
    expected: `1${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Column: array number accessor, Data: array",
  },
  {
    columns: ["value"],
    data: [{ value: "foo" }, { value: "bar" }],
    expected: `value${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Column: string accessor, Data: object",
  },
  {
    columns: [["value"]],
    data: [{ value: "foo" }, { value: "bar" }],
    expected: `value${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Column: array string accessor, Data: object",
  },
  {
    columns: [["msg", "value"]],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    expected: `value${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Column: array string accessor, Data: object",
  },
  {
    columns: [
      {
        header: "Value",
        prop: ["msg", "value"],
      },
    ],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    expected: `Value${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Explicit header",
  },
  {
    columns: [
      {
        fn: (str: string) => str.toUpperCase(),
        prop: ["msg", "value"],
      },
    ],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    expected: `value${NEWLINE}FOO${NEWLINE}BAR${NEWLINE}`,
    name: "[CSV_stringify] Transform function 1",
  },
  {
    columns: [
      {
        fn: (str: string) => Promise.resolve(str.toUpperCase()),
        prop: ["msg", "value"],
      },
    ],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    expected: `value${NEWLINE}FOO${NEWLINE}BAR${NEWLINE}`,
    name: "[CSV_stringify] Transform function 1 async",
  },
  {
    columns: [
      {
        fn: (obj: { value: string }) => obj.value,
        prop: "msg",
      },
    ],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    expected: `msg${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Transform function 2",
  },
  {
    columns: [
      {
        fn: (obj: { value: string }) => obj.value,
        header: "Value",
        prop: "msg",
      },
    ],
    data: [{ msg: { value: "foo" } }, { msg: { value: "bar" } }],
    expected: `Value${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Transform function 2, explicit header",
  },
  {
    columns: [0],
    data: [[{ value: "foo" }], [{ value: "bar" }]],
    expected:
      `0${NEWLINE}"{""value"":""foo""}"${NEWLINE}"{""value"":""bar""}"${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: object",
  },
  {
    columns: [0],
    data: [
      [[{ value: "foo" }, { value: "bar" }]],
      [[{ value: "baz" }, { value: "qux" }]],
    ],
    expected:
      `0${NEWLINE}"[{""value"":""foo""},{""value"":""bar""}]"${NEWLINE}"[{""value"":""baz""},{""value"":""qux""}]"${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: arary of objects",
  },
  {
    columns: [0],
    data: [[["foo", "bar"]], [["baz", "qux"]]],
    expected:
      `0${NEWLINE}"[""foo"",""bar""]"${NEWLINE}"[""baz"",""qux""]"${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: array",
  },
  {
    columns: [0],
    data: [[["foo", "bar"]], [["baz", "qux"]]],
    expected:
      `0${NEWLINE}"[""foo"",""bar""]"${NEWLINE}"[""baz"",""qux""]"${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: array, separator: tab",
    options: { separator: "\t" },
  },
  {
    columns: [0],
    data: [[], []],
    expected: `0${NEWLINE}${NEWLINE}${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: undefined",
  },
  {
    columns: [0],
    data: [[null], [null]],
    expected: `0${NEWLINE}${NEWLINE}${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: null",
  },
  {
    columns: [0],
    data: [[0xa], [0xb]],
    expected: `0${NEWLINE}10${NEWLINE}11${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: hex number",
  },
  {
    columns: [0],
    data: [[BigInt("1")], [BigInt("2")]],
    expected: `0${NEWLINE}1${NEWLINE}2${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: BigInt",
  },
  {
    columns: [0],
    data: [[true], [false]],
    expected: `0${NEWLINE}true${NEWLINE}false${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: boolean",
  },
  {
    columns: [0],
    data: [["foo"], ["bar"]],
    expected: `0${NEWLINE}foo${NEWLINE}bar${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: string",
  },
  {
    columns: [0],
    data: [[Symbol("foo")], [Symbol("bar")]],
    expected: `0${NEWLINE}Symbol(foo)${NEWLINE}Symbol(bar)${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: symbol",
  },
  {
    columns: [0],
    data: [[(n: number) => n]],
    expected: `0${NEWLINE}(n) => n${NEWLINE}`,
    name: "[CSV_stringify] Targeted value: function",
  },
  {
    columns: [0],
    data: [['foo"']],
    expected: `0${NEWLINE}"foo"""${NEWLINE}`,
    name: "[CSV_stringify] Value with double quote",
  },
  {
    columns: [0],
    data: [["foo\r\n"]],
    expected: `0${NEWLINE}"foo\r\n"${NEWLINE}`,
    name: "[CSV_stringify] Value with CRLF",
  },
  {
    columns: [0],
    data: [["foo\r"]],
    expected: `0${NEWLINE}foo\r${NEWLINE}`,
    name: "[CSV_stringify] Value with CR",
  },
  {
    columns: [0],
    data: [["foo\n"]],
    expected: `0${NEWLINE}foo\n${NEWLINE}`,
    name: "[CSV_stringify] Value with LF",
  },
  {
    columns: [0],
    data: [["foo,"]],
    expected: `0${NEWLINE}"foo,"${NEWLINE}`,
    name: "[CSV_stringify] Value with comma",
  },
  {
    columns: [0],
    data: [["foo,"]],
    expected: `0${NEWLINE}foo,${NEWLINE}`,
    name: "[CSV_stringify] Value with comma, tab separator",
    options: { separator: "\t" },
  },
];

for (const tc of stringifyTestCases) {
  if ((tc as StringifyTestCaseError).throwsError) {
    const t = tc as StringifyTestCaseError;
    Deno.test({
      async fn() {
        await assertThrowsAsync(
          async () => {
            await stringify(t.data, t.columns, t.options);
          },
          t.throwsError,
          t.errorMessage,
        );
      },
      name: t.name,
    });
  } else {
    const t = tc as StringifyTestCase;
    Deno.test({
      async fn() {
        const actual = await stringify(t.data, t.columns, t.options);
        assertEquals(actual, t.expected);
      },
      name: t.name,
    });
  }
}
