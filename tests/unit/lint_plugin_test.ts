// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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

function testVisit(source: string, ...selectors: string[]): string[] {
  const log: string[] = [];

  testPlugin(source, {
    create() {
      const visitor: LintVisitor = {};

      for (const s of selectors) {
        visitor[s] = () => log.push(s);
      }

      return visitor;
    },
  });

  return log;
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
  const enter = testVisit("foo", "Identifier");
  assertEquals(enter, ["Identifier"]);

  const exit = testVisit("foo", "Identifier:exit");
  assertEquals(exit, ["Identifier:exit"]);

  const both = testVisit("foo", "Identifier", "Identifier:exit");
  assertEquals(both, ["Identifier", "Identifier:exit"]);
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
