// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals } from "./test_util.ts";
import { assertSnapshot } from "@std/testing/snapshot";

// TODO(@marvinhagemeister) Remove once we land "official" types
export interface LintReportData {
  // deno-lint-ignore no-explicit-any
  node: any;
  message: string;
}
// TODO(@marvinhagemeister) Remove once we land "official" types
interface LintContext {
  id: string;
}
// TODO(@marvinhagemeister) Remove once we land "official" types
// deno-lint-ignore no-explicit-any
type LintVisitor = Record<string, (node: any) => void>;

// TODO(@marvinhagemeister) Remove once we land "official" types
interface LintRule {
  create(ctx: LintContext): LintVisitor;
  destroy?(): void;
}

// TODO(@marvinhagemeister) Remove once we land "official" types
interface LintPlugin {
  name: string;
  rules: Record<string, LintRule>;
}

function runLintPlugin(plugin: LintPlugin, fileName: string, source: string) {
  // deno-lint-ignore no-explicit-any
  return (Deno as any)[(Deno as any).internal].runLintPlugin(
    plugin,
    fileName,
    source,
  );
}

function testPlugin(
  source: string,
  rule: LintRule,
) {
  const plugin = {
    name: "test-plugin",
    rules: {
      testRule: rule,
    },
  };

  return runLintPlugin(plugin, "source.tsx", source);
}

interface VisitResult {
  selector: string;
  kind: "enter" | "exit";
  // deno-lint-ignore no-explicit-any
  node: any;
}

function testVisit(
  source: string,
  ...selectors: string[]
): VisitResult[] {
  const result: VisitResult[] = [];

  testPlugin(source, {
    create() {
      const visitor: LintVisitor = {};

      for (const s of selectors) {
        visitor[s] = (node) => {
          result.push({
            kind: s.endsWith(":exit") ? "exit" : "enter",
            selector: s,
            node,
          });
        };
      }

      return visitor;
    },
  });

  return result;
}

function testLintNode(source: string, ...selectors: string[]) {
  // deno-lint-ignore no-explicit-any
  const log: any[] = [];

  testPlugin(source, {
    create() {
      const visitor: LintVisitor = {};

      for (const s of selectors) {
        visitor[s] = (node) => {
          log.push(node[Symbol.for("Deno.lint.toJsValue")]());
        };
      }

      return visitor;
    },
  });

  assertEquals(log.length > 0, true);

  return log;
}

async function testSnapshot(
  t: Deno.TestContext,
  source: string,
  ...selectors: string[]
) {
  const res = testLintNode(source, ...selectors);
  await assertSnapshot(t, res[0]);
}

Deno.test("Plugin - visitor enter/exit", () => {
  const enter = testVisit(
    "foo",
    "Identifier",
  );
  assertEquals(enter[0].node.type, "Identifier");

  const exit = testVisit(
    "foo",
    "Identifier:exit",
  );
  assertEquals(exit[0].node.type, "Identifier");

  const both = testVisit("foo", "Identifier", "Identifier:exit");
  assertEquals(both.map((t) => t.selector), ["Identifier", "Identifier:exit"]);
});

Deno.test("Plugin - visitor descendant", () => {
  let result = testVisit(
    "if (false) foo; if (false) bar()",
    "IfStatement CallExpression",
  );
  assertEquals(result[0].node.type, "CallExpression");
  assertEquals(result[0].node.callee.name, "bar");

  result = testVisit(
    "if (false) foo; foo()",
    "IfStatement IfStatement",
  );
  assertEquals(result, []);

  result = testVisit(
    "if (false) foo; foo()",
    "* CallExpression",
  );
  assertEquals(result[0].node.type, "CallExpression");
});

Deno.test("Plugin - visitor child combinator", () => {
  let result = testVisit(
    "if (false) foo; if (false) { bar; }",
    "IfStatement > ExpressionStatement > Identifier",
  );
  assertEquals(result[0].node.name, "foo");

  result = testVisit(
    "if (false) foo; foo()",
    "IfStatement IfStatement",
  );
  assertEquals(result, []);
});

Deno.test("Plugin - visitor next sibling", () => {
  const result = testVisit(
    "if (false) foo; if (false) bar;",
    "IfStatement + IfStatement Identifier",
  );
  assertEquals(result[0].node.name, "bar");
});

Deno.test("Plugin - visitor subsequent sibling", () => {
  const result = testVisit(
    "if (false) foo; if (false) bar; if (false) baz;",
    "IfStatement ~ IfStatement Identifier",
  );
  assertEquals(result.map((r) => r.node.name), ["bar", "baz"]);
});

Deno.test("Plugin - visitor attr", () => {
  let result = testVisit(
    "for (const a of b) {}",
    "[await]",
  );
  assertEquals(result[0].node.await, false);

  result = testVisit(
    "for await (const a of b) {}",
    "[await=true]",
  );
  assertEquals(result[0].node.await, true);

  result = testVisit(
    "for await (const a of b) {}",
    "ForOfStatement[await=true]",
  );
  assertEquals(result[0].node.await, true);

  result = testVisit(
    "for (const a of b) {}",
    "ForOfStatement[await != true]",
  );
  assertEquals(result[0].node.await, false);

  result = testVisit(
    "async function *foo() {}",
    "FunctionDeclaration[async=true][generator=true]",
  );
  assertEquals(result[0].node.type, "FunctionDeclaration");

  result = testVisit(
    "foo",
    "[name='foo']",
  );
  assertEquals(result[0].node.name, "foo");
});

Deno.test("Plugin - visitor attr to check type", () => {
  let result = testVisit(
    "foo",
    "Identifier[type]",
  );
  assertEquals(result[0].node.type, "Identifier");

  result = testVisit(
    "foo",
    "Identifier[type='Identifier']",
  );
  assertEquals(result[0].node.type, "Identifier");
});

Deno.test("Plugin - visitor attr non-existing", () => {
  const result = testVisit(
    "foo",
    "[non-existing]",
  );
  assertEquals(result, []);
});

Deno.test("Plugin - visitor attr length special case", () => {
  let result = testVisit(
    "foo(1); foo(1, 2);",
    "CallExpression[arguments.length=2]",
  );
  assertEquals(result[0].node.arguments.length, 2);

  result = testVisit(
    "foo(1); foo(1, 2);",
    "CallExpression[arguments.length>1]",
  );
  assertEquals(result[0].node.arguments.length, 2);

  result = testVisit(
    "foo(1); foo(1, 2);",
    "CallExpression[arguments.length<2]",
  );
  assertEquals(result[0].node.arguments.length, 1);

  result = testVisit(
    "foo(1); foo(1, 2);",
    "CallExpression[arguments.length<=3]",
  );
  assertEquals(result[0].node.arguments.length, 1);
  assertEquals(result[1].node.arguments.length, 2);

  result = testVisit(
    "foo(1); foo(1, 2);",
    "CallExpression[arguments.length>=1]",
  );
  assertEquals(result[0].node.arguments.length, 1);
  assertEquals(result[1].node.arguments.length, 2);
});

Deno.test("Plugin - visitor :first-child", () => {
  const result = testVisit(
    "{ foo; bar }",
    "BlockStatement ExpressionStatement:first-child Identifier",
  );
  assertEquals(result[0].node.name, "foo");
});

Deno.test("Plugin - visitor :last-child", () => {
  const result = testVisit(
    "{ foo; bar }",
    "BlockStatement ExpressionStatement:last-child Identifier",
  );
  assertEquals(result[0].node.name, "bar");
});

Deno.test("Plugin - visitor :nth-child", () => {
  let result = testVisit(
    "{ foo; bar; baz; foobar; }",
    "BlockStatement ExpressionStatement:nth-child(2) Identifier",
  );
  assertEquals(result[0].node.name, "bar");

  result = testVisit(
    "{ foo; bar; baz; foobar; }",
    "BlockStatement ExpressionStatement:nth-child(2n) Identifier",
  );
  assertEquals(result[0].node.name, "foo");
  assertEquals(result[1].node.name, "baz");

  result = testVisit(
    "{ foo; bar; baz; foobar; }",
    "BlockStatement ExpressionStatement:nth-child(2n + 1) Identifier",
  );
  assertEquals(result[0].node.name, "bar");
  assertEquals(result[1].node.name, "foobar");

  result = testVisit(
    "{ foo; bar; baz; foobar; }",
    "BlockStatement *:nth-child(2n + 1 of ExpressionStatement) Identifier",
  );
  assertEquals(result[0].node.name, "bar");
  assertEquals(result[1].node.name, "foobar");
});

Deno.test("Plugin - Program", () => {
  const node = testLintNode("", "Program");
  assertEquals(node[0], {
    type: "Program",
    sourceType: "script",
    range: [1, 1],
    body: [],
  });
});

Deno.test("Plugin - ImportDeclaration", async (t) => {
  await testSnapshot(t, 'import "foo";', "ImportDeclaration");
  await testSnapshot(t, 'import foo from "foo";', "ImportDeclaration");
  await testSnapshot(t, 'import * as foo from "foo";', "ImportDeclaration");
  await testSnapshot(
    t,
    'import { foo, bar as baz } from "foo";',
    "ImportDeclaration",
  );
  await testSnapshot(
    t,
    'import foo from "foo" with { type: "json" };',
    "ImportDeclaration",
  );
});

Deno.test("Plugin - ExportNamedDeclaration", async (t) => {
  await testSnapshot(t, 'export foo from "foo";', "ExportNamedDeclaration");
  await testSnapshot(
    t,
    'export { foo, bar as baz } from "foo";',
    "ExportNamedDeclaration",
  );
  await testSnapshot(
    t,
    'export { foo } from "foo" with { type: "json" };',
    "ExportNamedDeclaration",
  );
});

Deno.test("Plugin - ExportDefaultDeclaration", async (t) => {
  await testSnapshot(
    t,
    "export default function foo() {}",
    "ExportDefaultDeclaration",
  );
  await testSnapshot(
    t,
    "export default function () {}",
    "ExportDefaultDeclaration",
  );
  await testSnapshot(
    t,
    "export default class Foo {}",
    "ExportDefaultDeclaration",
  );
  await testSnapshot(
    t,
    "export default class {}",
    "ExportDefaultDeclaration",
  );
  await testSnapshot(t, "export default bar;", "ExportDefaultDeclaration");
  await testSnapshot(
    t,
    "export default interface Foo {};",
    "ExportDefaultDeclaration",
  );
});

Deno.test("Plugin - ExportAllDeclaration", async (t) => {
  await testSnapshot(t, 'export * from "foo";', "ExportAllDeclaration");
  await testSnapshot(t, 'export * as foo from "foo";', "ExportAllDeclaration");
  await testSnapshot(
    t,
    'export * from "foo" with { type: "json" };',
    "ExportAllDeclaration",
  );
});

Deno.test("Plugin - BlockStatement", async (t) => {
  await testSnapshot(t, "{ foo; }", "BlockStatement");
});

Deno.test("Plugin - BreakStatement", async (t) => {
  await testSnapshot(t, "break;", "BreakStatement");
  await testSnapshot(t, "break foo;", "BreakStatement");
});

Deno.test("Plugin - ContinueStatement", async (t) => {
  await testSnapshot(t, "continue;", "ContinueStatement");
  await testSnapshot(t, "continue foo;", "ContinueStatement");
});

Deno.test("Plugin - DebuggerStatement", async (t) => {
  await testSnapshot(t, "debugger;", "DebuggerStatement");
});

Deno.test("Plugin - DoWhileStatement", async (t) => {
  await testSnapshot(t, "do {} while (foo);", "DoWhileStatement");
});

Deno.test("Plugin - ExpressionStatement", async (t) => {
  await testSnapshot(t, "foo;", "ExpressionStatement");
});

Deno.test("Plugin - ForInStatement", async (t) => {
  await testSnapshot(t, "for (a in b) {}", "ForInStatement");
});

Deno.test("Plugin - ForOfStatement", async (t) => {
  await testSnapshot(t, "for (a of b) {}", "ForOfStatement");
  await testSnapshot(t, "for await (a of b) {}", "ForOfStatement");
});

Deno.test("Plugin - ForStatement", async (t) => {
  await testSnapshot(t, "for (;;) {}", "ForStatement");
  await testSnapshot(t, "for (a; b; c) {}", "ForStatement");
});

Deno.test("Plugin - IfStatement", async (t) => {
  await testSnapshot(t, "if (foo) {}", "IfStatement");
  await testSnapshot(t, "if (foo) {} else {}", "IfStatement");
});

Deno.test("Plugin - LabeledStatement", async (t) => {
  await testSnapshot(t, "foo: {};", "LabeledStatement");
});

Deno.test("Plugin - ReturnStatement", async (t) => {
  await testSnapshot(t, "return", "ReturnStatement");
  await testSnapshot(t, "return foo;", "ReturnStatement");
});

Deno.test("Plugin - SwitchStatement", async (t) => {
  await testSnapshot(
    t,
    `switch (foo) {
      case foo:
      case bar:
        break;
      default:
        {}
    }`,
    "SwitchStatement",
  );
});

Deno.test("Plugin - ThrowStatement", async (t) => {
  await testSnapshot(t, "throw foo;", "ThrowStatement");
});

Deno.test("Plugin - TryStatement", async (t) => {
  await testSnapshot(t, "try {} catch {};", "TryStatement");
  await testSnapshot(t, "try {} catch (e) {};", "TryStatement");
  await testSnapshot(t, "try {} finally {};", "TryStatement");
});

Deno.test("Plugin - WhileStatement", async (t) => {
  await testSnapshot(t, "while (foo) {}", "WhileStatement");
});

Deno.test("Plugin - WithStatement", async (t) => {
  await testSnapshot(t, "with ([]) {}", "WithStatement");
});

Deno.test("Plugin - ArrayExpression", async (t) => {
  await testSnapshot(t, "[[],,[]]", "ArrayExpression");
});

Deno.test("Plugin - ArrowFunctionExpression", async (t) => {
  await testSnapshot(t, "() => {}", "ArrowFunctionExpression");
  await testSnapshot(t, "async () => {}", "ArrowFunctionExpression");
  await testSnapshot(
    t,
    "(a: number, ...b: any[]): any => {}",
    "ArrowFunctionExpression",
  );
});

Deno.test("Plugin - AssignmentExpression", async (t) => {
  await testSnapshot(t, "a = b", "AssignmentExpression");
  await testSnapshot(t, "a = a ??= b", "AssignmentExpression");
});

Deno.test("Plugin - AwaitExpression", async (t) => {
  await testSnapshot(t, "await foo;", "AwaitExpression");
});

Deno.test("Plugin - BinaryExpression", async (t) => {
  await testSnapshot(t, "a > b", "BinaryExpression");
  await testSnapshot(t, "a >= b", "BinaryExpression");
  await testSnapshot(t, "a < b", "BinaryExpression");
  await testSnapshot(t, "a <= b", "BinaryExpression");
  await testSnapshot(t, "a == b", "BinaryExpression");
  await testSnapshot(t, "a === b", "BinaryExpression");
  await testSnapshot(t, "a != b", "BinaryExpression");
  await testSnapshot(t, "a !== b", "BinaryExpression");
  await testSnapshot(t, "a << b", "BinaryExpression");
  await testSnapshot(t, "a >> b", "BinaryExpression");
  await testSnapshot(t, "a >>> b", "BinaryExpression");
  await testSnapshot(t, "a + b", "BinaryExpression");
  await testSnapshot(t, "a - b", "BinaryExpression");
  await testSnapshot(t, "a * b", "BinaryExpression");
  await testSnapshot(t, "a / b", "BinaryExpression");
  await testSnapshot(t, "a % b", "BinaryExpression");
  await testSnapshot(t, "a | b", "BinaryExpression");
  await testSnapshot(t, "a ^ b", "BinaryExpression");
  await testSnapshot(t, "a & b", "BinaryExpression");
  await testSnapshot(t, "a in b", "BinaryExpression");
  await testSnapshot(t, "a ** b", "BinaryExpression");
});

Deno.test("Plugin - CallExpression", async (t) => {
  await testSnapshot(t, "foo();", "CallExpression");
  await testSnapshot(t, "foo(a, ...b);", "CallExpression");
  await testSnapshot(t, "foo?.();", "CallExpression");
  await testSnapshot(t, "foo<T>();", "CallExpression");
});

Deno.test("Plugin - ChainExpression", async (t) => {
  const node = testLintNode("a?.b", "ChainExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ClassExpression", async (t) => {
  await testSnapshot(t, "a = class {}", "ClassExpression");
  await testSnapshot(t, "a = class Foo {}", "ClassExpression");
  await testSnapshot(t, "a = class Foo extends Bar {}", "ClassExpression");
  await testSnapshot(
    t,
    "a = class Foo extends Bar implements Baz, Baz2 {}",
    "ClassExpression",
  );
  await testSnapshot(t, "a = class Foo<T> {}", "ClassExpression");
  await testSnapshot(t, "a = class { foo() {} }", "ClassExpression");
  await testSnapshot(t, "a = class { #foo() {} }", "ClassExpression");
  await testSnapshot(t, "a = class { foo: number }", "ClassExpression");
  await testSnapshot(t, "a = class { foo = bar }", "ClassExpression");
  await testSnapshot(
    t,
    "a = class { constructor(public foo: string) {} }",
    "ClassExpression",
  );
  await testSnapshot(t, "a = class { #foo: number = bar }", "ClassExpression");
  await testSnapshot(t, "a = class { static foo = bar }", "ClassExpression");
  await testSnapshot(
    t,
    "a = class { static foo; static { foo = bar } }",
    "ClassExpression",
  );
});

Deno.test("Plugin - ConditionalExpression", async (t) => {
  await testSnapshot(t, "a ? b : c", "ConditionalExpression");
});

Deno.test("Plugin - FunctionExpression", async (t) => {
  await testSnapshot(t, "a = function () {}", "FunctionExpression");
  await testSnapshot(t, "a = function foo() {}", "FunctionExpression");
  await testSnapshot(
    t,
    "a = function (a?: number, ...b: any[]): any {}",
    "FunctionExpression",
  );
  await testSnapshot(t, "a = async function* () {}", "FunctionExpression");
});

Deno.test("Plugin - Identifier", async (t) => {
  await testSnapshot(t, "a", "Identifier");
});

Deno.test("Plugin - ImportExpression", async (t) => {
  await testSnapshot(
    t,
    "import('foo', { with: { type: 'json' }}",
    "ImportExpression",
  );
});

Deno.test("Plugin - LogicalExpression", async (t) => {
  await testSnapshot(t, "a && b", "LogicalExpression");
  await testSnapshot(t, "a || b", "LogicalExpression");
  await testSnapshot(t, "a ?? b", "LogicalExpression");
});

Deno.test("Plugin - MemberExpression", async (t) => {
  await testSnapshot(t, "a.b", "MemberExpression");
  await testSnapshot(t, "a['b']", "MemberExpression");
});

Deno.test("Plugin - MetaProp", async (t) => {
  await testSnapshot(t, "import.meta", "MetaProp");
});

Deno.test("Plugin - NewExpression", async (t) => {
  await testSnapshot(t, "new Foo()", "NewExpression");
  await testSnapshot(t, "new Foo(a?: any, ...b: any[])", "NewExpression");
});

Deno.test("Plugin - ObjectExpression", async (t) => {
  await testSnapshot(t, "{}", "ObjectExpression");
  await testSnapshot(t, "{ a, b: c, [c]: d }", "ObjectExpression");
});

Deno.test("Plugin - PrivateIdentifier", async (t) => {
  await testSnapshot(t, "class Foo { #foo = foo }", "PrivateIdentifier");
});

Deno.test("Plugin - SequenceExpression", async (t) => {
  await testSnapshot(t, "(a, b)", "SequenceExpression");
});

Deno.test("Plugin - Super", async (t) => {
  await testSnapshot(
    t,
    "class Foo extends Bar { constructor() { super(); } }",
    "Super",
  );
});

Deno.test("Plugin - TaggedTemplateExpression", async (t) => {
  await testSnapshot(t, "foo`foo ${bar} baz`", "TaggedTemplateExpression");
});

Deno.test("Plugin - TemplateLiteral", async (t) => {
  await testSnapshot(t, "`foo ${bar} baz`", "TemplateLiteral");
});

Deno.test("Plugin - ThisExpression", async (t) => {
  await testSnapshot(t, "this", "ThisExpression");
});

Deno.test("Plugin - TSAsExpression", async (t) => {
  await testSnapshot(t, "a as b", "TSAsExpression");
  await testSnapshot(t, "a as const", "TSAsExpression");
});

Deno.test("Plugin - TSNonNullExpression", async (t) => {
  const node = testLintNode("a!", "TSNonNullExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TSSatisfiesExpression", async (t) => {
  await testSnapshot(t, "a satisfies b", "TSSatisfiesExpression");
});

Deno.test("Plugin - UnaryExpression", async (t) => {
  await testSnapshot(t, "typeof a", "UnaryExpression");
  await testSnapshot(t, "void 0", "UnaryExpression");
  await testSnapshot(t, "-a", "UnaryExpression");
  await testSnapshot(t, "+a", "UnaryExpression");
});

Deno.test("Plugin - UpdateExpression", async (t) => {
  await testSnapshot(t, "a++", "UpdateExpression");
  await testSnapshot(t, "++a", "UpdateExpression");
  await testSnapshot(t, "a--", "UpdateExpression");
  await testSnapshot(t, "--a", "UpdateExpression");
});

Deno.test("Plugin - YieldExpression", async (t) => {
  await testSnapshot(t, "function* foo() { yield bar; }", "YieldExpression");
});

Deno.test("Plugin - Literal", async (t) => {
  await testSnapshot(t, "1", "Literal");
  await testSnapshot(t, "'foo'", "Literal");
  await testSnapshot(t, '"foo"', "Literal");
  await testSnapshot(t, "true", "Literal");
  await testSnapshot(t, "false", "Literal");
  await testSnapshot(t, "null", "Literal");
  await testSnapshot(t, "1n", "Literal");
  await testSnapshot(t, "/foo/g", "Literal");
});

Deno.test("Plugin - TS Interface", async (t) => {
  await testSnapshot(t, "interface A {}", "TSInterface");
  await testSnapshot(t, "interface A<T> {}", "TSInterface");
  await testSnapshot(t, "interface A extends Foo<T>, Bar<T> {}", "TSInterface");
  await testSnapshot(t, "interface A { foo: any, bar?: any }", "TSInterface");
  await testSnapshot(
    t,
    "interface A { readonly [key: string]: any }",
    "TSInterface",
  );

  await testSnapshot(t, "interface A { readonly a: any }", "TSInterface");
  await testSnapshot(t, "interface A { <T>(a: T): T }", "TSInterface");
  await testSnapshot(t, "interface A { new <T>(a: T): T }", "TSInterface");
  await testSnapshot(t, "interface A { a: new <T>(a: T) => T }", "TSInterface");
});
