#!/usr/bin/env -S deno run -A
// Copyright 2018-2026 the Deno authors. MIT license.
//
// Axis-2 `declare class` -> `interface` + `declare var` conversion. See
// docs/designs/deno-libs-generator.md and denoland/deno#36094.
//
// Deno declares some web-platform types as `declare class X { ... }` where every
// stock lib uses `interface X { ... }` + `declare var X: { ... }`. A class and
// an interface of the same name CANNOT merge, so co-loading `lib.dom` yields
// `TS2300: Duplicate identifier`. This splits each such class into its two-axis
// shape:
//
//   * the instance members become `interface X` - a pure type that declaration-
//     merges with `lib.dom`'s interface (Axis 2, no false positive: an interface
//     asserts nothing about globals);
//   * the constructor and static members become `declare var X: { ... }` - the
//     global *binding*, which the Axis-1 deferral (tools/apply_web_globals_
//     deferral.ts) then defers to `lib.dom` when present.
//
// Membership is unchanged: Deno still declares exactly the same globals it did
// (the class already WAS the value binding), just as a `var` instead of a
// `class`. The target set is derived - Deno `declare class` names that stock
// `lib.dom` declares as an `interface` - so it stays in sync on TS bumps.
//
// Run:  export DENO_TSC_BIN=$(deno run -A tools/download_tsc.ts)
//       deno run -A tools/defork_classes.ts
//       deno run -A tools/apply_web_globals_deferral.ts   # defer the new vars
//
// deno-lint-ignore-file no-console

import { Project } from "jsr:@ts-morph/ts-morph@27.0.2";

const dtsDir = new URL("../cli/tsc/dts/", import.meta.url).pathname;

const denoTscBin = Deno.env.get("DENO_TSC_BIN");
if (!denoTscBin) {
  throw new Error(
    "DENO_TSC_BIN is not set. Run:\n" +
      "  export DENO_TSC_BIN=$(deno run -A tools/download_tsc.ts)",
  );
}
const stockDir = denoTscBin.slice(0, denoTscBin.lastIndexOf("/") + 1);

const project = new Project({ useInMemoryFileSystem: false });
const domInterfaces = new Set(
  project.addSourceFileAtPath(stockDir + "lib.dom.d.ts")
    .getInterfaces().map((i) => i.getName()),
);

// `extends A implements B` -> `extends A, B`; `implements B` -> `extends B`.
function interfaceHeritage(ext: string | undefined, impls: string[]): string {
  const parts = [ext, ...impls].filter((s): s is string => !!s);
  return parts.length ? ` extends ${parts.join(", ")}` : "";
}

const converted: string[] = [];

for (const entry of Deno.readDirSync(dtsDir)) {
  if (!/^lib\.deno.*\.d\.ts$/.test(entry.name)) continue;
  const sf = project.addSourceFileAtPath(dtsDir + entry.name);
  const classes = sf.getClasses().filter((c) => {
    const n = c.getName();
    return !!n && domInterfaces.has(n);
  });
  if (classes.length === 0) continue;

  let text = sf.getFullText();
  const edits: Array<[number, number, string]> = [];

  for (const cls of classes) {
    const name = cls.getName()!;
    if (cls.getTypeParameters().length) {
      throw new Error(`${name}: type parameters are unhandled`);
    }

    const heritage = interfaceHeritage(
      cls.getExtends()?.getText(),
      cls.getImplements().map((i) => i.getText()),
    );

    // Var members: statics (verbatim) + a `new(...)` per constructor + prototype.
    const ctors = cls.getConstructors();
    const statics = cls.getStaticMembers();
    const varMembers: string[] = [];
    for (const s of statics) {
      varMembers.push(s.getText().replace(/^static\s+/, ""));
    }
    if (ctors.length === 0) {
      varMembers.push(`new (): ${name}`);
    } else {
      for (const c of ctors) {
        const params = c.getParameters().map((p) => p.getText()).join(", ");
        varMembers.push(`new (${params}): ${name}`);
      }
    }
    varMembers.push(`readonly prototype: ${name}`);

    // Instance members become the interface body, byte-for-byte (JSDoc,
    // indentation preserved). Collect their source spans; anything NOT an
    // instance member (constructors, statics) is dropped from the class body.
    const instance = cls.getInstanceMembers();
    const bodyParts: string[] = [];
    for (const m of instance) {
      // getStart(true) includes leading JSDoc; walk back to the line start to
      // keep indentation.
      let start = m.getStart(true);
      while (start > 0 && text[start - 1] !== "\n") start--;
      bodyParts.push(text.slice(start, m.getEnd()));
    }
    const body = bodyParts.join("\n");

    const iface = `interface ${name}${heritage} {\n${body}\n}`;
    const varDecl = `declare var ${name}: {\n  ${
      varMembers.join(";\n  ")
    };\n};`;

    edits.push([cls.getStart(false), cls.getEnd(), `${iface}\n\n${varDecl}`]);
    converted.push(`${entry.name}:${name}`);
  }

  edits.sort((a, b) => b[0] - a[0]);
  for (const [start, end, repl] of edits) {
    text = text.slice(0, start) + repl + text.slice(end);
  }
  Deno.writeTextFileSync(dtsDir + entry.name, text);
}

console.log(`converted ${converted.length} class(es) to interface + var:`);
console.log(converted.sort().map((s) => `  ${s}`).join("\n"));
