#!/usr/bin/env -S deno run --allow-env --allow-read --allow-write=.
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  ModuleDeclarationKind,
  Node,
  Project,
  ts,
} from "jsr:@ts-morph/ts-morph@23";
import { Path } from "jsr:@david/path@0.2";

const dir = new Path(import.meta.dirname!);
const typesNodeDir = dir.join("../../../DefinitelyTyped/types/node");

const project = new Project({
  tsConfigFilePath: typesNodeDir.join("tsconfig.json").toString(),
});
const names = new Set<string>();
const ignoredNames = new Set<string>([
  "Array",
  "BigInt64Array",
  "BigUint64Array",
  "Float32Array",
  "Float64Array",
  "Int16Array",
  "Int32Array",
  "Int8Array",
  "NodeJS",
  "ReadonlyArray",
  "RelativeIndexable",
  "RequireResolve",
  "String",
  "SymbolConstructor",
  "Uint16Array",
  "Uint32Array",
  "Uint8Array",
  "Uint8ClampedArray",
  "WithImplicitCoercion",
]);

for (const file of project.getSourceFiles()) {
  for (
    const mod of file.getDescendantsOfKind(ts.SyntaxKind.ModuleDeclaration)
  ) {
    if (mod.getDeclarationKind() !== ModuleDeclarationKind.Global) continue;

    for (const statement of mod.getStatements()) {
      if (Node.isVariableStatement(statement)) {
        for (const decl of statement.getDeclarations()) {
          if (ignoredNames.has(decl.getName())) continue;
          names.add(decl.getName());
        }
      } else if (Node.hasName(statement)) {
        if (ignoredNames.has(statement.getName())) continue;
        names.add(statement.getName());
      }
    }
  }
}

// deno-lint-ignore no-console
console.log(
  "Globals: ",
  Array.from(names).sort(),
);
