// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../assert/mod.ts";
import {
  ArrayValue,
  BareKey,
  BasicString,
  DateTime,
  DottedKey,
  Float,
  InlineTable,
  Integer,
  LiteralString,
  LocalTime,
  MultilineBasicString,
  MultilineLiteralString,
  Pair,
  ParserFactory,
  Scanner,
  Symbols,
  Table,
  TOMLParseError,
  Utils,
  Value,
} from "./_parser.ts";
import { parse } from "./parse.ts";

Deno.test({
  name: "[TOML parser] Scanner",
  fn() {
    const scanner = new Scanner(" # comment\n\n\na \nb");
    scanner.nextUntilChar({ inline: true });
    assertEquals(scanner.char(), "#");
    scanner.nextUntilChar();
    assertEquals(scanner.char(), "a");
    scanner.next();
    scanner.nextUntilChar({ inline: true });
    assertEquals(scanner.char(), "\n");
    scanner.nextUntilChar();
    assertEquals(scanner.char(), "b");
    scanner.next();
    assertEquals(scanner.eof(), true);
  },
});

Deno.test({
  name: "[TOML parser] bare key",
  fn() {
    const parse = ParserFactory(BareKey);
    assertEquals(parse("A-Za-z0-9_-"), "A-Za-z0-9_-");
    assertThrows(() => parse(""));
    assertThrows(() => parse('"foo"'));
  },
});

Deno.test({
  name: "[TOML parser] basic string",
  fn() {
    const parse = ParserFactory(BasicString);
    assertEquals(
      parse('"a\\"\\n\\t\\b\\\\\\u3042\\U01F995"'),
      'a"\n\t\b\\\あ🦕',
    );
    assertEquals(parse('""'), "");
    assertThrows(() => parse(""));
    assertThrows(() => parse('"a'));
    assertThrows(() => parse('"a\nb"'));
  },
});

Deno.test({
  name: "[TOML parser] literal string",
  fn() {
    const parse = ParserFactory(LiteralString);
    assertEquals(parse("'a\\n'"), "a\\n");
    assertThrows(() => parse(""));
    assertThrows(() => parse("'a"));
    assertThrows(() => parse("a\nb"));
  },
});

Deno.test({
  name: "[TOML parser] multi-line basic string",
  fn() {
    const parse = ParserFactory(MultilineBasicString);
    assertEquals(
      parse(`"""
Roses are red
Violets are\\tblue"""`),
      "Roses are red\nViolets are\tblue",
    );
    assertEquals(
      parse(`"""\\
    The quick brown \\
    fox jumps over \\
    the lazy dog.\\
    """`),
      "The quick brown fox jumps over the lazy dog.",
    );
  },
});

Deno.test({
  name: "[TOML parser] multi-line literal string",
  fn() {
    const parse = ParserFactory(MultilineLiteralString);
    assertEquals(
      parse(`'''
Roses are red
Violets are\\tblue'''`),
      "Roses are red\nViolets are\\tblue",
    );
  },
});

Deno.test({
  name: "[TOML parser] symbols",
  fn() {
    const parse = ParserFactory(Symbols);
    assertEquals(parse("true"), true);
    assertEquals(parse("nan"), NaN);
    assertEquals(parse("inf"), Infinity);
    assertThrows(() => parse(""));
    assertThrows(() => parse("_"));
  },
});

Deno.test({
  name: "[TOML parser] dotted key",
  fn() {
    const parse = ParserFactory(DottedKey);
    assertEquals(parse("a . b . c"), ["a", "b", "c"]);
    assertEquals(parse(`a.'b.c'."d.e"`), ["a", "b.c", "d.e"]);
    assertThrows(() => parse(""));
    assertThrows(() => parse("a.b ."));
    assertThrows(() => parse("."));
  },
});

Deno.test({
  name: "[TOML parser] table",
  fn() {
    const parse = ParserFactory(Table);
    assertEquals(
      parse(`
[foo.bar]
baz = true
fizz.buzz = true
`.trim()),
      {
        type: "Table",
        key: ["foo", "bar"],
        value: {
          baz: true,
          fizz: {
            buzz: true,
          },
        },
      },
    );
    assertEquals(parse(`[only.header]`), {
      type: "Table",
      key: ["only", "header"],
      value: {},
    });
    assertThrows(() => parse(""));
    assertThrows(() => parse("["));
    assertThrows(() => parse("[o"));
  },
});

Deno.test({
  name: "[TOML parser] integer",
  fn() {
    const parse = ParserFactory(Integer);
    assertEquals(parse("123"), 123);
    assertEquals(parse("+123"), 123);
    assertEquals(parse("-123"), -123);
    assertEquals(parse("123_456"), 123456);
    assertEquals(parse("0xDEADBEEF"), "0xDEADBEEF");
    assertEquals(parse("0xdeadbeef"), "0xdeadbeef");
    assertEquals(parse("0xdead_beef"), "0xdead_beef");
    assertEquals(parse("0o01234567"), "0o01234567");
    assertEquals(parse("0o755"), "0o755");
    assertEquals(parse("0b11010110"), "0b11010110");
    assertThrows(() => parse(""));
    assertThrows(() => parse("+Z"));
    assertThrows(() => parse("0x"));
  },
});

Deno.test({
  name: "[TOML parser] float",
  fn() {
    const parse = ParserFactory(Float);
    assertEquals(parse("+1.0"), 1.0);
    assertEquals(parse("3.1415"), 3.1415);
    assertEquals(parse("-0.01"), -0.01);
    assertEquals(parse("5e+22"), 5e+22);
    assertEquals(parse("1e06"), 1e06);
    assertEquals(parse("-2E-2"), -2E-2);
    assertEquals(parse("6.626e-34"), 6.626e-34);
    assertEquals(parse("224_617.445_991_228"), 224_617.445_991_228);
    assertThrows(() => parse(""));
    assertThrows(() => parse("X"));
  },
});

Deno.test({
  name: "[TOML parser] date and date time",
  fn() {
    const parse = ParserFactory(DateTime);
    assertEquals(
      parse("1979-05-27T07:32:00Z"),
      new Date("1979-05-27T07:32:00Z"),
    );
    assertEquals(
      parse("1979-05-27T00:32:00-07:00"),
      new Date("1979-05-27T07:32:00Z"),
    );
    assertEquals(
      parse("1979-05-27T00:32:00.999999-07:00"),
      new Date("1979-05-27T07:32:00.999Z"),
    );
    assertEquals(
      parse("1979-05-27 07:32:00Z"),
      new Date("1979-05-27T07:32:00Z"),
    );
    assertEquals(parse("1979-05-27T07:32:00"), new Date("1979-05-27T07:32:00"));
    assertEquals(
      parse("1979-05-27T00:32:00.999999"),
      new Date("1979-05-27T00:32:00.999999"),
    );
    assertEquals(parse("1979-05-27"), new Date("1979-05-27"));
    assertThrows(() => parse(""));
    assertThrows(() => parse("X"));
    assertThrows(() => parse("0000-00-00"));
  },
});

Deno.test({
  name: "[TOML parser] local time",
  fn() {
    const parse = ParserFactory(LocalTime);
    assertEquals(parse("07:32:00"), "07:32:00");
    assertEquals(parse("07:32:00.999"), "07:32:00.999");
    assertThrows(() => parse(""));
  },
});

Deno.test({
  name: "[TOML parser] value",
  fn() {
    const parse = ParserFactory(Value);
    assertEquals(parse("1"), 1);
    assertEquals(parse("1.2"), 1.2);
    assertEquals(parse("1979-05-27"), new Date("1979-05-27"));
    assertEquals(parse("07:32:00"), "07:32:00");
    assertEquals(parse(`"foo.com"`), "foo.com");
    assertEquals(parse(`'foo.com'`), "foo.com");
  },
});

Deno.test({
  name: "[TOML parser] key value pair",
  fn() {
    const parse = ParserFactory(Pair);
    assertEquals(parse("key = 'value'"), { key: "value" });
    assertThrows(() => parse("key ="));
    assertThrows(() => parse("key = \n 'value'"));
    assertThrows(() => parse("key \n = 'value'"));
  },
});

Deno.test({
  name: "[TOML parser] array",
  fn() {
    const parse = ParserFactory(ArrayValue);
    assertEquals(parse("[]"), []);
    assertEquals(parse("[1, 2, 3]"), [1, 2, 3]);
    assertEquals(parse(`[ "red", "yellow", "green" ]`), [
      "red",
      "yellow",
      "green",
    ]);
    assertEquals(parse(`[ [ 1, 2 ], [3, 4, 5] ]`), [[1, 2], [3, 4, 5]]);
    assertEquals(parse(`[ [ 1, 2 ], ["a", "b", "c"] ]`), [
      [1, 2],
      ["a", "b", "c"],
    ]);
    assertEquals(
      parse(`[
      { x = 1, y = 2, z = 3 },
      { x = 7, y = 8, z = 9 },
      { x = 2, y = 4, z = 8 }
    ]`),
      [{ x: 1, y: 2, z: 3 }, { x: 7, y: 8, z: 9 }, { x: 2, y: 4, z: 8 }],
    );
    assertEquals(
      parse(`[ # comment
        1, # comment
        2, # this is ok
      ]`),
      [1, 2],
    );
  },
});

Deno.test({
  name: "[TOML parser] inline table",
  fn() {
    const parse = ParserFactory(InlineTable);
    assertEquals(parse(`{ first = "Tom", last = "Preston-Werner" }`), {
      first: "Tom",
      last: "Preston-Werner",
    });
    assertEquals(parse(`{ type.name = "pug" }`), { type: { name: "pug" } });
    assertThrows(() => parse(`{ x = 1`));
    assertThrows(() => parse(`{ x = 1,\n y = 2 }`));
    assertThrows(() => parse(`{ x = 1, }`));
  },
});

Deno.test({
  name: "[TOML parser] Utils.deepAssignWithTable",
  fn() {
    const source = {
      foo: {
        items: [
          {
            id: "a",
          },
          {
            id: "b",
            profile: {
              name: "b",
            },
          },
        ],
      },
    };

    Utils.deepAssignWithTable(
      source,
      {
        type: "Table",
        key: ["foo", "items", "profile", "email", "x"],
        value: { main: "mail@example.com" },
      },
    );
    assertEquals(
      source,
      {
        foo: {
          items: [
            {
              id: "a",
            },
            {
              id: "b",
              profile: {
                name: "b",
                email: {
                  x: { main: "mail@example.com" },
                },
              } as unknown,
            },
          ],
        },
      },
    );
  },
});

Deno.test({
  name: "[TOML parser] Utils.deepAssignWithTable / TableArray",
  fn() {
    const source = {
      foo: {},
    };

    Utils.deepAssignWithTable(
      source,
      {
        type: "TableArray",
        key: ["foo", "items"],
        value: { email: "mail@example.com" },
      },
    );
    assertEquals(
      source,
      {
        foo: {
          items: [
            {
              email: "mail@example.com",
            },
          ],
        },
      },
    );
    Utils.deepAssignWithTable(
      source,
      {
        type: "TableArray",
        key: ["foo", "items"],
        value: { email: "sub@example.com" },
      },
    );
    assertEquals(
      source,
      {
        foo: {
          items: [
            {
              email: "mail@example.com",
            },
            {
              email: "sub@example.com",
            },
          ],
        },
      },
    );
  },
});

Deno.test({
  name: "[TOML parser] error message",
  fn() {
    assertThrows(
      () => parse("foo = 1\nbar ="),
      TOMLParseError,
      "on line 2, column 5",
    );
    assertThrows(
      () => parse("foo = 1\nbar = 'foo\nbaz=1"),
      TOMLParseError,
      "line 2, column 10",
    );
  },
});
