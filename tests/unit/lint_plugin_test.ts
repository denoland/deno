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

Deno.test("Plugin - BlockStatement", async (t) => {
  const node = testLintNode("{ foo; }", "BlockStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - BreakStatement", async (t) => {
  let node = testLintNode("break;", "BreakStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("break foo;", "BreakStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ContinueStatement", async (t) => {
  let node = testLintNode("continue;", "ContinueStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("continue foo;", "ContinueStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - DebuggerStatement", async (t) => {
  const node = testLintNode("debugger;", "DebuggerStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - DoWhileStatement", async (t) => {
  const node = testLintNode("do {} while (foo);", "DoWhileStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ExpressionStatement", async (t) => {
  const node = testLintNode("foo;", "ExpressionStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ForInStatement", async (t) => {
  const node = testLintNode("for (a in b) {}", "ForInStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ForOfStatement", async (t) => {
  let node = testLintNode("for (a of b) {}", "ForOfStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("for await (a of b) {}", "ForOfStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ForStatement", async (t) => {
  let node = testLintNode("for (;;) {}", "ForStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("for (a; b; c) {}", "ForStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - IfStatement", async (t) => {
  let node = testLintNode("if (foo) {}", "IfStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("if (foo) {} else {}", "IfStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - LabeledStatement", async (t) => {
  const node = testLintNode("foo: {};", "LabeledStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ReturnStatement", async (t) => {
  let node = testLintNode("return", "ReturnStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("return foo;", "ReturnStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - SwitchStatement", async (t) => {
  const node = testLintNode(
    `switch (foo) {
      case foo:
      case bar:
        break;
      default:
        {}
    }`,
    "SwitchStatement",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ThrowStatement", async (t) => {
  const node = testLintNode("throw foo;", "ThrowStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TryStatement", async (t) => {
  let node = testLintNode("try {} catch {};", "TryStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("try {} catch (e) {};", "TryStatement");
  await assertSnapshot(t, node[0]);

  node = testLintNode("try {} finally {};", "TryStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - WhileStatement", async (t) => {
  const node = testLintNode("while (foo) {}", "WhileStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - WithStatement", async (t) => {
  const node = testLintNode("with ([]) {}", "WithStatement");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ArrayExpression", async (t) => {
  const node = testLintNode("[[],,[]]", "ArrayExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ArrowFunctionExpression", async (t) => {
  let node = testLintNode("() => {}", "ArrowFunctionExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("async () => {}", "ArrowFunctionExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode(
    "(a: number, ...b: any[]): any => {}",
    "ArrowFunctionExpression",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - AssignmentExpression", async (t) => {
  let node = testLintNode("a = b", "AssignmentExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = a ??= b", "AssignmentExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - AwaitExpression", async (t) => {
  const node = testLintNode("await foo;", "AwaitExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - BinaryExpression", async (t) => {
  let node = testLintNode("a > b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a >= b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a < b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a <= b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a == b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a === b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a != b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a !== b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a << b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a >> b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a >>> b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a + b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a - b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a * b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a / b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a % b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a | b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a ^ b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a & b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a in b", "BinaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a ** b", "BinaryExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - CallExpression", async (t) => {
  let node = testLintNode("foo();", "CallExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("foo(a, ...b);", "CallExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("foo?.();", "CallExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ChainExpression", async (t) => {
  const node = testLintNode("a?.b", "ChainExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ClassExpression", async (t) => {
  let node = testLintNode("a = class {}", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class Foo {}", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class Foo extends Bar {}", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode(
    "a = class Foo extends Bar implements Baz, Baz2 {}",
    "ClassExpression",
  );
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class Foo<T> {}", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class { foo() {} }", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class { #foo() {} }", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class { foo: number }", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class { foo = bar }", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode(
    "a = class { constructor(public foo: string) {} }",
    "ClassExpression",
  );
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class { #foo: number = bar }", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = class { static foo = bar }", "ClassExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode(
    "a = class { static foo; static { foo = bar } }",
    "ClassExpression",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ConditionalExpression", async (t) => {
  const node = testLintNode("a ? b : c", "ConditionalExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - FunctionExpression", async (t) => {
  let node = testLintNode("a = function () {}", "FunctionExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = function foo() {}", "FunctionExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode(
    "a = function (a?: number, ...b: any[]): any {}",
    "FunctionExpression",
  );
  await assertSnapshot(t, node[0]);

  node = testLintNode("a = async function* () {}", "FunctionExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - Identifier", async (t) => {
  const node = testLintNode("a", "Identifier");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ImportExpression", async (t) => {
  const node = testLintNode(
    "import('foo', { with: { type: 'json' }}",
    "ImportExpression",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - LogicalExpression", async (t) => {
  let node = testLintNode("a && b", "LogicalExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a || b", "LogicalExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a ?? b", "LogicalExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - MemberExpression", async (t) => {
  let node = testLintNode("a.b", "MemberExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a['b']", "MemberExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - MetaProp", async (t) => {
  const node = testLintNode("import.meta", "MetaProp");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - NewExpression", async (t) => {
  let node = testLintNode("new Foo()", "NewExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("new Foo(a?: any, ...b: any[])", "NewExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ObjectExpression", async (t) => {
  let node = testLintNode("{}", "ObjectExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("{ a, b: c, [c]: d }", "ObjectExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - PrivateIdentifier", async (t) => {
  const node = testLintNode("class Foo { #foo = foo }", "PrivateIdentifier");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - SequenceExpression", async (t) => {
  const node = testLintNode("(a, b)", "SequenceExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - Super", async (t) => {
  const node = testLintNode(
    "class Foo extends Bar { constructor() { super(); } }",
    "Super",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TaggedTemplateExpression", async (t) => {
  const node = testLintNode(
    "foo`foo ${bar} baz`",
    "TaggedTemplateExpression",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TemplateLiteral", async (t) => {
  const node = testLintNode(
    "`foo ${bar} baz`",
    "TemplateLiteral",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - ThisExpression", async (t) => {
  const node = testLintNode("this", "ThisExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TSAsExpression", async (t) => {
  let node = testLintNode("a as b", "TSAsExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a as const", "TSAsExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TSNonNullExpression", async (t) => {
  const node = testLintNode("a!", "TSNonNullExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - TSSatisfiesExpression", async (t) => {
  const node = testLintNode("a satisfies b", "TSSatisfiesExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - UnaryExpression", async (t) => {
  let node = testLintNode("typeof a", "UnaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("void 0", "UnaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("-a", "UnaryExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("+a", "UnaryExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - UpdateExpression", async (t) => {
  let node = testLintNode("a++", "UpdateExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("++a", "UpdateExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("a--", "UpdateExpression");
  await assertSnapshot(t, node[0]);

  node = testLintNode("--a", "UpdateExpression");
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - YieldExpression", async (t) => {
  const node = testLintNode(
    "function* foo() { yield bar; }",
    "YieldExpression",
  );
  await assertSnapshot(t, node[0]);
});

Deno.test("Plugin - Literal", async (t) => {
  let node = testLintNode("1", "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode("'foo'", "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode('"foo"', "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode("true", "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode("false", "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode("null", "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode("1n", "Literal");
  await assertSnapshot(t, node[0]);

  node = testLintNode("/foo/g", "Literal");
  await assertSnapshot(t, node[0]);
});
