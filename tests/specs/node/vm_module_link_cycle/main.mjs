// Exercises `vm.SourceTextModule.prototype.link(linker)` with dependency
// graphs that the old single-pass linker could not handle: a diamond
// (shared dependency) and a true import cycle (A imports B, B imports A).
// Mirrors the spirit of upstream Node's test/parallel/test-vm-module-link.js.
import assert from "node:assert";
import { SourceTextModule, SyntheticModule } from "node:vm";

// Diamond: root imports `a` and `b`; both import `shared`. The `shared`
// module must be instantiated exactly once and reused for both edges.
{
  let sharedEvaluations = 0;
  const shared = new SourceTextModule(
    "globalThis.__diamondShared = (globalThis.__diamondShared ?? 0) + 1; export const v = 'shared';",
    { identifier: "shared" },
  );
  const a = new SourceTextModule(
    "import { v } from 'shared'; export const a = v + ':a';",
    { identifier: "a" },
  );
  const b = new SourceTextModule(
    "import { v } from 'shared'; export const b = v + ':b';",
    { identifier: "b" },
  );
  const root = new SourceTextModule(
    "import { a } from 'a'; import { b } from 'b'; export const result = a + '|' + b;",
    { identifier: "root" },
  );

  await root.link((specifier) => {
    if (specifier === "a") return a;
    if (specifier === "b") return b;
    if (specifier === "shared") return shared;
    throw new Error(`unexpected specifier ${specifier}`);
  });
  await root.evaluate();

  assert.strictEqual(root.namespace.result, "shared:a|shared:b");
  // The shared module's body runs once even though two modules import it.
  sharedEvaluations = globalThis.__diamondShared;
  assert.strictEqual(sharedEvaluations, 1);
  console.log("ok diamond");
}

// Cycle: `a` imports `b`, `b` imports `a`. Linking must terminate (the old
// implementation recursed forever) and both namespaces must resolve.
{
  const a = new SourceTextModule(
    "import { fromB } from 'b'; export const fromA = 'A'; export function getB() { return fromB; }",
    { identifier: "cycle-a" },
  );
  const b = new SourceTextModule(
    "import { fromA } from 'a'; export const fromB = 'B'; export function getA() { return fromA; }",
    { identifier: "cycle-b" },
  );

  await a.link((specifier) => {
    if (specifier === "a") return a;
    if (specifier === "b") return b;
    throw new Error(`unexpected specifier ${specifier}`);
  });
  await a.evaluate();

  assert.strictEqual(a.namespace.fromA, "A");
  assert.strictEqual(a.namespace.getB(), "B");
  assert.strictEqual(b.namespace.fromB, "B");
  assert.strictEqual(b.namespace.getA(), "A");
  console.log("ok cycle");
}

// A synthetic module reached through the link graph is left alone (it is
// already linked on construction) and its exports are visible.
{
  const synthetic = new SyntheticModule(["answer"], function () {
    this.setExport("answer", 42);
  }, { identifier: "synth" });
  const root = new SourceTextModule(
    "import { answer } from 'synth'; export const doubled = answer * 2;",
    { identifier: "uses-synth" },
  );

  await root.link((specifier) => {
    if (specifier === "synth") return synthetic;
    throw new Error(`unexpected specifier ${specifier}`);
  });
  await root.evaluate();

  assert.strictEqual(root.namespace.doubled, 84);
  console.log("ok shared-synthetic");
}
