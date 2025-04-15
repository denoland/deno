// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals } from "@std/assert/equals";
import {
  ATTR_BIN_NODE,
  ATTR_EXISTS_NODE,
  BinOp,
  ELEM_NODE,
  FIELD_NODE,
  Lexer,
  parseSelector,
  PSEUDO_FIRST_CHILD,
  PSEUDO_HAS,
  PSEUDO_IS,
  PSEUDO_LAST_CHILD,
  PSEUDO_NOT,
  PSEUDO_NTH_CHILD,
  RELATION_NODE,
  splitSelectors,
  Token,
} from "../../cli/js/40_lint_selector.js";
import { assertThrows } from "@std/assert";

Deno.test("splitSelectors", () => {
  assertEquals(splitSelectors("*"), ["*"]);
  assertEquals(splitSelectors("*,*"), ["*", "*"]);
  assertEquals(splitSelectors("*,*     "), ["*", "*"]);
  assertEquals(splitSelectors("foo"), ["foo"]);
  assertEquals(splitSelectors("foo, bar"), ["foo", "bar"]);
  assertEquals(splitSelectors("foo:f(bar, baz)"), ["foo:f(bar, baz)"]);
  assertEquals(splitSelectors("foo:f(bar, baz), foobar"), [
    "foo:f(bar, baz)",
    "foobar",
  ]);
});

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
  assertEquals(testLexer("Foo"), [
    { token: Token.Word, value: "Foo" },
  ]);
  assertEquals(testLexer("foo-bar"), [
    { token: Token.Word, value: "foo-bar" },
  ]);
  assertEquals(testLexer("foo_bar"), [
    { token: Token.Word, value: "foo_bar" },
  ]);
  assertEquals(testLexer("Foo Bar Baz"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Bar" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Baz" },
  ]);
  assertEquals(testLexer("Foo   Bar   Baz"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Bar" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Baz" },
  ]);
});

Deno.test("Lexer - Relation >", () => {
  assertEquals(testLexer("Foo > Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "Bar" },
  ]);
  assertEquals(testLexer("Foo>Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "Bar" },
  ]);
  assertEquals(testLexer(">Bar"), [
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "Bar" },
  ]);
});

Deno.test("Lexer - Relation +", () => {
  assertEquals(testLexer("Foo + Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "Bar" },
  ]);
  assertEquals(testLexer("Foo+Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "Bar" },
  ]);
  assertEquals(testLexer("+Bar"), [
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "Bar" },
  ]);
});

Deno.test("Lexer - Relation ~", () => {
  assertEquals(testLexer("Foo ~ Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);
  assertEquals(testLexer("Foo~Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);
  assertEquals(testLexer("~Bar"), [
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);

  assertEquals(testLexer("Foo Bar ~ Bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Space, value: "" },
    { token: Token.Word, value: "Bar" },
    { token: Token.Op, value: "~" },
    { token: Token.Word, value: "Bar" },
  ]);
});

Deno.test("Lexer - Attr", () => {
  assertEquals(testLexer("[attr]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr=1]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "=" },
    { token: Token.Word, value: "1" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr='foo']"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "=" },
    { token: Token.String, value: "foo" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr>=2]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: ">=" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr<=2]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "<=" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr>2]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: ">" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr<2]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "<" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr!=2]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Op, value: "!=" },
    { token: Token.Word, value: "2" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr.foo=1]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.Dot, value: "" },
    { token: Token.Word, value: "foo" },
    { token: Token.Op, value: "=" },
    { token: Token.Word, value: "1" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("[attr] [attr]"), [
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
    { token: Token.Space, value: "" },
    { token: Token.BracketOpen, value: "" },
    { token: Token.Word, value: "attr" },
    { token: Token.BracketClose, value: "" },
  ]);
  assertEquals(testLexer("Foo[attr][attr2=1]"), [
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
  assertEquals(testLexer(":foo-bar"), [
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
  ]);
  assertEquals(testLexer("Foo:foo-bar"), [
    { token: Token.Word, value: "Foo" },
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
  ]);
  assertEquals(testLexer(":foo-bar(baz)"), [
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
    { token: Token.BraceOpen, value: "" },
    { token: Token.Word, value: "baz" },
    { token: Token.BraceClose, value: "" },
  ]);
  assertEquals(testLexer(":foo-bar(2n + 1)"), [
    { token: Token.Colon, value: "" },
    { token: Token.Word, value: "foo-bar" },
    { token: Token.BraceOpen, value: "" },
    { token: Token.Word, value: "2n" },
    { token: Token.Op, value: "+" },
    { token: Token.Word, value: "1" },
    { token: Token.BraceClose, value: "" },
  ]);
});

Deno.test("Lexer - field", () => {
  assertEquals(testLexer(".bar"), [
    { token: Token.Dot, value: "" },
    { token: Token.Word, value: "bar" },
  ]);
  assertEquals(testLexer(".bar.baz"), [
    { token: Token.Dot, value: "" },
    { token: Token.Word, value: "bar" },
    { token: Token.Dot, value: "" },
    { token: Token.Word, value: "baz" },
  ]);
});

Deno.test("Parser - Elem", () => {
  assertEquals(testParse("Foo"), [[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
  ]]);
});

Deno.test("Parser - Relation (descendant)", () => {
  assertEquals(testParse("Foo Bar"), [[
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
  assertEquals(testParse("Foo > Bar"), [[
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

  assertEquals(testParse("Foo ~ Bar"), [[
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

  assertEquals(testParse("Foo + Bar"), [[
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

Deno.test("Parser - Field", () => {
  assertEquals(testParse("Foo.bar"), [[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    { type: FIELD_NODE, props: [2] },
  ]]);
  assertEquals(testParse("Foo .bar"), [[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    { type: FIELD_NODE, props: [2] },
  ]]);
  assertEquals(testParse("Foo .foo.bar"), [[
    {
      type: ELEM_NODE,
      elem: 1,
      wildcard: false,
    },
    { type: FIELD_NODE, props: [1, 2] },
  ]]);
});

Deno.test("Parser - Attr", () => {
  assertEquals(testParse("[foo]"), [[
    {
      type: ATTR_EXISTS_NODE,
      prop: [1],
    },
  ]]);

  assertEquals(testParse("[foo][bar]"), [[
    {
      type: ATTR_EXISTS_NODE,
      prop: [1],
    },
    {
      type: ATTR_EXISTS_NODE,
      prop: [2],
    },
  ]]);

  assertEquals(testParse("[foo=1]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: 1,
    },
  ]]);
  assertEquals(testParse("[foo=true]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: true,
    },
  ]]);
  assertEquals(testParse("[foo=false]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: false,
    },
  ]]);
  assertEquals(testParse("[foo=null]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: null,
    },
  ]]);
  assertEquals(testParse("[foo='str']"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: "str",
    },
  ]]);
  assertEquals(testParse('[foo="str"]'), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: "str",
    },
  ]]);
  assertEquals(testParse("[foo=/str/]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: /str/,
    },
  ]]);
  assertEquals(testParse("[foo=/str/g]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1],
      value: /str/g,
    },
  ]]);
});

Deno.test("Parser - Attr nested", () => {
  assertEquals(testParse("[foo.bar]"), [[
    {
      type: ATTR_EXISTS_NODE,
      prop: [1, 2],
    },
  ]]);

  assertEquals(testParse("[foo.bar = 2]"), [[
    {
      type: ATTR_BIN_NODE,
      op: BinOp.Equal,
      prop: [1, 2],
      value: 2,
    },
  ]]);
});

Deno.test("Parser - Pseudo no value", () => {
  assertEquals(testParse(":first-child"), [[
    {
      type: PSEUDO_FIRST_CHILD,
    },
  ]]);
  assertEquals(testParse(":last-child"), [[
    {
      type: PSEUDO_LAST_CHILD,
    },
  ]]);
});

Deno.test("Parser - Pseudo nth-child", () => {
  assertEquals(testParse(":nth-child(2)"), [[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: null,
      step: 0,
      stepOffset: 1,
      repeat: false,
    },
  ]]);
  assertEquals(testParse(":nth-child(2n)"), [[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: null,
      step: 2,
      stepOffset: 0,
      repeat: true,
    },
  ]]);
  assertEquals(testParse(":nth-child(-2n)"), [[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: null,
      step: -2,
      stepOffset: 0,
      repeat: true,
    },
  ]]);
  assertEquals(testParse(":nth-child(2n + 1)"), [[
    {
      type: PSEUDO_NTH_CHILD,
      of: null,
      op: "+",
      step: 2,
      stepOffset: 1,
      repeat: true,
    },
  ]]);
  assertEquals(testParse(":nth-child(2n + 1 of Foo[attr])"), [[
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
  assertThrows(() => testParse(":nth-child(2n + 1 of Foo[attr], Bar)"));
  assertThrows(() => testParse(":nth-child(2n - 1 foo)"));
});

Deno.test("Parser - Pseudo :has()", () => {
  assertEquals(testParse(":has(Foo:has(Foo), Bar)"), [[
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

Deno.test("Parser - Pseudo :is()/:where()/:matches()", () => {
  assertEquals(testParse(":is(Foo:is(Foo), Bar)"), [[
    {
      type: PSEUDO_IS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_IS,
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
  assertEquals(testParse(":where(Foo:where(Foo), Bar)"), [[
    {
      type: PSEUDO_IS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_IS,
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
  assertEquals(testParse(":matches(Foo:matches(Foo), Bar)"), [[
    {
      type: PSEUDO_IS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 1, wildcard: false },
          {
            type: PSEUDO_IS,
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

  assertEquals(testParse("Foo:is(Bar)"), [[
    { type: ELEM_NODE, elem: 1, wildcard: false },
    {
      type: PSEUDO_IS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 2, wildcard: false },
        ],
      ],
    },
  ]]);

  assertEquals(testParse("Foo :is(Bar)"), [[
    { type: ELEM_NODE, elem: 1, wildcard: false },
    { type: RELATION_NODE, op: BinOp.Space },
    {
      type: PSEUDO_IS,
      selectors: [
        [
          { type: ELEM_NODE, elem: 2, wildcard: false },
        ],
      ],
    },
  ]]);
});

Deno.test("Parser - Pseudo not", () => {
  assertEquals(testParse(":not(Foo:not(Foo), Bar)"), [[
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
  assertEquals(testParse("Foo[foo=true] Bar"), [[
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
