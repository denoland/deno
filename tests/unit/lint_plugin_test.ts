// Copyright 2018-2025 the Deno authors. MIT license.

import { assertEquals } from "./test_util.ts";

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

Deno.test("Plugin - BlockStatement", () => {
  const node = testLintNode("{ foo; }", "BlockStatement");
  assertEquals(node[0], {
    type: "BlockStatement",
    range: [1, 9],
    body: [{
      type: "ExpressionStatement",
      range: [3, 7],
      expression: {
        type: "Identifier",
        name: "foo",
        range: [3, 6],
      },
    }],
  });
});

Deno.test("Plugin - BreakStatement", () => {
  let node = testLintNode("break;", "BreakStatement");
  assertEquals(node[0], {
    type: "BreakStatement",
    range: [1, 7],
    label: null,
  });

  node = testLintNode("break foo;", "BreakStatement");
  assertEquals(node[0], {
    type: "BreakStatement",
    range: [1, 11],
    label: {
      type: "Identifier",
      range: [7, 10],
      name: "foo",
    },
  });
});

Deno.test("Plugin - ContinueStatement", () => {
  let node = testLintNode("continue;", "ContinueStatement");
  assertEquals(node[0], {
    type: "ContinueStatement",
    range: [1, 10],
    label: null,
  });

  node = testLintNode("continue foo;", "ContinueStatement");
  assertEquals(node[0], {
    type: "ContinueStatement",
    range: [1, 14],
    label: {
      type: "Identifier",
      range: [10, 13],
      name: "foo",
    },
  });
});

Deno.test("Plugin - DebuggerStatement", () => {
  const node = testLintNode("debugger;", "DebuggerStatement");
  assertEquals(node[0], {
    type: "DebuggerStatement",
    range: [1, 10],
  });
});

Deno.test("Plugin - DoWhileStatement", () => {
  const node = testLintNode("do {} while (foo);", "DoWhileStatement");
  assertEquals(node[0], {
    type: "DoWhileStatement",
    range: [1, 19],
    test: {
      type: "Identifier",
      range: [14, 17],
      name: "foo",
    },
    body: {
      type: "BlockStatement",
      range: [4, 6],
      body: [],
    },
  });
});

Deno.test("Plugin - ExpressionStatement", () => {
  const node = testLintNode("foo;", "ExpressionStatement");
  assertEquals(node[0], {
    type: "ExpressionStatement",
    range: [1, 5],
    expression: {
      type: "Identifier",
      range: [1, 4],
      name: "foo",
    },
  });
});

Deno.test("Plugin - ForInStatement", () => {
  const node = testLintNode("for (a in b) {}", "ForInStatement");
  assertEquals(node[0], {
    type: "ForInStatement",
    range: [1, 16],
    left: {
      type: "Identifier",
      range: [6, 7],
      name: "a",
    },
    right: {
      type: "Identifier",
      range: [11, 12],
      name: "b",
    },
    body: {
      type: "BlockStatement",
      range: [14, 16],
      body: [],
    },
  });
});

Deno.test("Plugin - ForOfStatement", () => {
  let node = testLintNode("for (a of b) {}", "ForOfStatement");
  assertEquals(node[0], {
    type: "ForOfStatement",
    range: [1, 16],
    await: false,
    left: {
      type: "Identifier",
      range: [6, 7],
      name: "a",
    },
    right: {
      type: "Identifier",
      range: [11, 12],
      name: "b",
    },
    body: {
      type: "BlockStatement",
      range: [14, 16],
      body: [],
    },
  });

  node = testLintNode("for await (a of b) {}", "ForOfStatement");
  assertEquals(node[0], {
    type: "ForOfStatement",
    range: [1, 22],
    await: true,
    left: {
      type: "Identifier",
      range: [12, 13],
      name: "a",
    },
    right: {
      type: "Identifier",
      range: [17, 18],
      name: "b",
    },
    body: {
      type: "BlockStatement",
      range: [20, 22],
      body: [],
    },
  });
});

Deno.test("Plugin - ForStatement", () => {
  let node = testLintNode("for (;;) {}", "ForStatement");
  assertEquals(node[0], {
    type: "ForStatement",
    range: [1, 12],
    init: null,
    test: null,
    update: null,
    body: {
      type: "BlockStatement",
      range: [10, 12],
      body: [],
    },
  });

  node = testLintNode("for (a; b; c) {}", "ForStatement");
  assertEquals(node[0], {
    type: "ForStatement",
    range: [1, 17],
    init: {
      type: "Identifier",
      range: [6, 7],
      name: "a",
    },
    test: {
      type: "Identifier",
      range: [9, 10],
      name: "b",
    },
    update: {
      type: "Identifier",
      range: [12, 13],
      name: "c",
    },
    body: {
      type: "BlockStatement",
      range: [15, 17],
      body: [],
    },
  });
});

Deno.test("Plugin - IfStatement", () => {
  let node = testLintNode("if (foo) {}", "IfStatement");
  assertEquals(node[0], {
    type: "IfStatement",
    range: [1, 12],
    test: {
      type: "Identifier",
      name: "foo",
      range: [5, 8],
    },
    consequent: {
      type: "BlockStatement",
      range: [10, 12],
      body: [],
    },
    alternate: null,
  });

  node = testLintNode("if (foo) {} else {}", "IfStatement");
  assertEquals(node[0], {
    type: "IfStatement",
    range: [1, 20],
    test: {
      type: "Identifier",
      name: "foo",
      range: [5, 8],
    },
    consequent: {
      type: "BlockStatement",
      range: [10, 12],
      body: [],
    },
    alternate: {
      type: "BlockStatement",
      range: [18, 20],
      body: [],
    },
  });
});

Deno.test("Plugin - LabeledStatement", () => {
  const node = testLintNode("foo: {};", "LabeledStatement");
  assertEquals(node[0], {
    type: "LabeledStatement",
    range: [1, 8],
    label: {
      type: "Identifier",
      name: "foo",
      range: [1, 4],
    },
    body: {
      type: "BlockStatement",
      range: [6, 8],
      body: [],
    },
  });
});

Deno.test("Plugin - ReturnStatement", () => {
  let node = testLintNode("return", "ReturnStatement");
  assertEquals(node[0], {
    type: "ReturnStatement",
    range: [1, 7],
    argument: null,
  });

  node = testLintNode("return foo;", "ReturnStatement");
  assertEquals(node[0], {
    type: "ReturnStatement",
    range: [1, 12],
    argument: {
      type: "Identifier",
      name: "foo",
      range: [8, 11],
    },
  });
});

Deno.test("Plugin - SwitchStatement", () => {
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
  assertEquals(node[0], {
    type: "SwitchStatement",
    range: [1, 94],
    discriminant: {
      type: "Identifier",
      range: [9, 12],
      name: "foo",
    },
    cases: [
      {
        type: "SwitchCase",
        range: [22, 31],
        test: {
          type: "Identifier",
          range: [27, 30],
          name: "foo",
        },
        consequent: [],
      },
      {
        type: "SwitchCase",
        range: [38, 62],
        test: {
          type: "Identifier",
          range: [43, 46],
          name: "bar",
        },
        consequent: [
          {
            type: "BreakStatement",
            label: null,
            range: [56, 62],
          },
        ],
      },
      {
        type: "SwitchCase",
        range: [69, 88],
        test: null,
        consequent: [
          {
            type: "BlockStatement",
            range: [86, 88],
            body: [],
          },
        ],
      },
    ],
  });
});

Deno.test("Plugin - ThrowStatement", () => {
  const node = testLintNode("throw foo;", "ThrowStatement");
  assertEquals(node[0], {
    type: "ThrowStatement",
    range: [1, 11],
    argument: {
      type: "Identifier",
      range: [7, 10],
      name: "foo",
    },
  });
});

Deno.test("Plugin - TryStatement", () => {
  let node = testLintNode("try {} catch {};", "TryStatement");
  assertEquals(node[0], {
    type: "TryStatement",
    range: [1, 16],
    block: {
      type: "BlockStatement",
      range: [5, 7],
      body: [],
    },
    handler: {
      type: "CatchClause",
      range: [8, 16],
      param: null,
      body: {
        type: "BlockStatement",
        range: [14, 16],
        body: [],
      },
    },
    finalizer: null,
  });

  node = testLintNode("try {} catch (e) {};", "TryStatement");
  assertEquals(node[0], {
    type: "TryStatement",
    range: [1, 20],
    block: {
      type: "BlockStatement",
      range: [5, 7],
      body: [],
    },
    handler: {
      type: "CatchClause",
      range: [8, 20],
      param: {
        type: "Identifier",
        range: [15, 16],
        name: "e",
      },
      body: {
        type: "BlockStatement",
        range: [18, 20],
        body: [],
      },
    },
    finalizer: null,
  });

  node = testLintNode("try {} finally {};", "TryStatement");
  assertEquals(node[0], {
    type: "TryStatement",
    range: [1, 18],
    block: {
      type: "BlockStatement",
      range: [5, 7],
      body: [],
    },
    handler: null,
    finalizer: {
      type: "BlockStatement",
      range: [16, 18],
      body: [],
    },
  });
});

Deno.test("Plugin - WhileStatement", () => {
  const node = testLintNode("while (foo) {}", "WhileStatement");
  assertEquals(node[0], {
    type: "WhileStatement",
    range: [1, 15],
    test: {
      type: "Identifier",
      range: [8, 11],
      name: "foo",
    },
    body: {
      type: "BlockStatement",
      range: [13, 15],
      body: [],
    },
  });
});
