// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  ATTR_BIN_NODE,
  ATTR_EXISTS_NODE,
  BinOp,
  ELEM_NODE,
  Lexer,
  parseSelector,
  PSEUDO_FIRST_CHILD,
  PSEUDO_HAS,
  PSEUDO_LAST_CHILD,
  PSEUDO_NOT,
  PSEUDO_NTH_CHILD,
  RELATION_NODE,
  Token,
} from "./40_lint_selector.js";
import { expect } from "@std/expect";

interface LexState {
  token: number;
  value: string;
}

function testLexer(input: string): LexState[] {
  const out: LexState[] = [];
  const l = new Lexer(input);

  while (l.token !== Token.EOF) {
    out.push({ token: l.token, value: l.value });
    l.next();
  }

  return out;
}

const Tags: Record<string, number> = { Foo: 1, Bar: 2, FooBar: 3 };
const Attrs: Record<string, number> = { foo: 1, bar: 2, foobar: 3, attr: 4 };
const toTag = (name: string): number => Tags[name];
const toAttr = (name: string): number => Attrs[name];

const testParse = (input: string) => parseSelector(input, toTag, toAttr);

Deno.test("Lexer - Elem", () => {
  expect(testLexer("Foo")).toEqual([
    { token: Token.Word, value: "Foo" },
  ]);
  expect(testLexer("foo-bar")).toEqual([
    { token: Token.Word, value: "foo-bar" },
  ]);
  expect(testLexer("foo_bar")).toEqual([
    { token: Token.Word, value: "foo_bar" },
  ]);
  expect(testLexer("Foo Bar Baz")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Bar" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Baz" },
  ]);
  expect(testLexer("Foo   Bar   Baz")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Bar" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Baz" },
  ]);
});

Deno.test("Lexer - Relation >", () => {
  expect(testLexer("Foo > Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "Bar" },
  ]);
  expect(testLexer("Foo>Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "Bar" },
  ]);
  expect(testLexer(">Bar")).toEqual([
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "Bar" },
  ]);
});

Deno.test("Lexer - Relation +", () => {
  expect(testLexer("Foo + Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "Bar" },
  ]);
  expect(testLexer("Foo+Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "Bar" },
  ]);
  expect(testLexer("+Bar")).toEqual([
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "Bar" },
  ]);
});

Deno.test("Lexer - Relation ~", () => {
  expect(testLexer("Foo ~ Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);
  expect(testLexer("Foo~Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);
  expect(testLexer("~Bar")).toEqual([
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);

  expect(testLexer("Foo Bar ~ Bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Bar" },
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);
});

Deno.test("Lexer - Attr", () => {
  expect(testLexer("[attr]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr=1]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "=" },
    { token: Token.Word, value: "1" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr='foo']")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "=" },
    { token: Token.String, value: "foo" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr>=2]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: ">=" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr<=2]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "<=" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr>2]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr<2]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "<" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr!=2]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "!=" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr.foo=1]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Dot, value: "" },
    { token: Token.Word, value: "foo" },
    { token: Token.Op, value: "=" },
    { token: Token.Word, value: "1" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("[attr] [attr]")).toEqual([
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
    { token: Token.Space, value: "" },
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
  ]);
  expect(testLexer("Foo[attr][attr2=1]")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr2" },
    { token: Token.Op, value: "=" },
    { token: Token.Word, value: "1" },
    { token: Token.BracketClose, value: "" },
  ]);
});

Deno.test("Lexer - Pseudo", () => {
  expect(testLexer(":foo-bar")).toEqual([
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
  ]);
  expect(testLexer("Foo:foo-bar")).toEqual([
    { token: Token.Word, value: "Foo" },
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
  ]);
  expect(testLexer(":foo-bar(baz)")).toEqual([
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
    { token: Token.BraceOpen, value: "" },
    { token: Token.Word, value: "baz" },
    { token: Token.BraceClose, value: "" },
  ]);
  expect(testLexer(":foo-bar(2n + 1)")).toEqual([
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
    { token: Token.BraceOpen, value: "" },
    { token: Token.Word, value: "2n" },
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "1" },
    { token: Token.BraceClose, value: "" },
  ]);
});

Deno.test("Parser - Elem", () => {
  expect(testParse("Foo")).toEqual([[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
  ]]);
});

Deno.test("Parser - Relation (descendant)", () => {
  expect(testParse("Foo Bar")).toEqual([[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    {
      type: RELATION_NODE,
      op: BinOp.Space,
    },
    {
      type: ELEM_NODE,
      elem: 2,
      wildcard: false,
    },
  ]]);
});

Deno.test("Parser - Relation", () => {
  expect(testParse("Foo > Bar")).toEqual([[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    {
      type: RELATION_NODE,
      op: BinOp.Greater,
    },
    {
      type: ELEM_NODE,
      elem: 2,
      wildcard: false,
    },
  ]]);

  expect(testParse("Foo ~ Bar")).toEqual([[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    {
      type: RELATION_NODE,
      op: BinOp.Tilde,
    },
    {
      type: ELEM_NODE,
      elem: 2,
      wildcard: false,
    },
  ]]);

  expect(testParse("Foo + Bar")).toEqual([[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    {
      type: RELATION_NODE,
      op: BinOp.Plus,
    },
    {
      type: ELEM_NODE,
      elem: 2,
      wildcard: false,
    },
  ]]);
});

Deno.test("Parser - Attr", () => {
  expect(testParse("[foo]")).toEqual([[
    {
      type: ATTR_EXISTS_NODE,
      prop: [1],
    },
  ]]);

  expect(testParse("[foo][bar]")).toEqual([[
    {
      type: ATTR_EXISTS_NODE,
      prop: [1],
    },
    {
      type: ATTR_EXISTS_NODE,
      prop: [2],
    },
  ]]);

  expect(testParse("[foo=1]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: 1,
    },
  ]]);
  expect(testParse("[foo=true]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: true,
    },
  ]]);
  expect(testParse("[foo=false]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: false,
    },
  ]]);
  expect(testParse("[foo=null]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: null,
    },
  ]]);
  expect(testParse("[foo='str']")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: "str",
    },
  ]]);
  expect(testParse('[foo="str"]')).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: "str",
    },
  ]]);
  expect(testParse("[foo=/str/]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: /str/,
    },
  ]]);
  expect(testParse("[foo=/str/g]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: /str/g,
    },
  ]]);
});

Deno.test("Parser - Attr nested", () => {
  expect(testParse("[foo.bar]")).toEqual([[
    {
      type: ATTR_EXISTS_NODE,
      prop: [1, 2],
    },
  ]]);

  expect(testParse("[foo.bar = 2]")).toEqual([[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1, 2],
      value: 2,
    },
  ]]);
});

Deno.test("Parser - Pseudo no value", () => {
  expect(testParse(":first-child")).toEqual([[
    {
      type: PSEUDO_FIRST_CHILD,
    },
  ]]);
  expect(testParse(":last-child")).toEqual([[
    {
      type: PSEUDO_LAST_CHILD,
    },
  ]]);
});

Deno.test("Parser - Pseudo nth-child", () => {
  expect(testParse(":nth-child(2)")).toEqual([[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: null,
      step: 0,
      stepOffset: 1,
      repeat: false,
    },
  ]]);
  expect(testParse(":nth-child(2n)")).toEqual([[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: null,
      step: 2,
      stepOffset: 0,
      repeat: true,
    },
  ]]);
  expect(testParse(":nth-child(-2n)")).toEqual([[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: null,
      step: -2,
      stepOffset: 0,
      repeat: true,
    },
  ]]);
  expect(testParse(":nth-child(2n + 1)")).toEqual([[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: "+",
      step: 2,
      stepOffset: 1,
      repeat: true,
    },
  ]]);
  expect(testParse(":nth-child(2n + 1 of Foo[attr])"))
    .toEqual([[
      {
        type: PSEUDO_NTH_CHILD,
        of: [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          { type: ATTR_EXISTS_NODE, prop: [4] },
        ],
        op: "+",
        step: 2,
        stepOffset: 1,
        repeat: true,
      },
    ]]);

  // Invalid selectors
  expect(() => testParse(":nth-child(2n + 1 of Foo[attr], Bar)"))
    .toThrow();
  expect(() => testParse(":nth-child(2n - 1 foo)")).toThrow();
});

Deno.test("Parser - Pseudo has/is/where", () => {
  expect(testParse(":has(Foo:has(Foo), Bar)")).toEqual([[
    {
      type: PSEUDO_HAS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_HAS,
            selectors: [
              [{ type: ELEM_NODE, elem: 1, wildcard: false }],
            ],
          },
        ],
        [
          { type: ELEM_NODE, elem: 2, wildcard: false },
        ],
      ],
    },
  ]]);
  expect(testParse(":where(Foo:where(Foo), Bar)")).toEqual([[
    {
      type: PSEUDO_HAS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_HAS,
            selectors: [
              [{ type: ELEM_NODE, elem: 1, wildcard: false }],
            ],
          },
        ],
        [
          { type: ELEM_NODE, elem: 2, wildcard: false },
        ],
      ],
    },
  ]]);
  expect(testParse(":is(Foo:is(Foo), Bar)")).toEqual([[
    {
      type: PSEUDO_HAS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_HAS,
            selectors: [
              [{ type: ELEM_NODE, elem: 1, wildcard: false }],
            ],
          },
        ],
        [
          { type: ELEM_NODE, elem: 2, wildcard: false },
        ],
      ],
    },
  ]]);
});

Deno.test("Parser - Pseudo not", () => {
  expect(testParse(":not(Foo:not(Foo), Bar)")).toEqual([[
    {
      type: PSEUDO_NOT,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_NOT,
            selectors: [
              [{ type: ELEM_NODE, elem: 1, wildcard: false }],
            ],
          },
        ],
        [
          { type: ELEM_NODE, elem: 2, wildcard: false },
        ],
      ],
    },
  ]]);
});

Deno.test("Parser - mixed", () => {
  expect(testParse("Foo[foo=true] Bar")).toEqual([[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    { type: ATTR_BIN_NODE, op: BinOp.Equal, prop: [1], value: true },
    { type: RELATION_NODE, op: BinOp.Space },
    {
      type: ELEM_NODE,
      elem: 2,
      wildcard: false,
    },
  ]]);
});
