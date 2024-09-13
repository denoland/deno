#!/usr/bin/env -S deno run --allow-env --allow-read --allow-write=.
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
const bannableNames = new Set<string>([
  "structuredClone",
  "AsyncDisposable",
  "Disposable",
  "ImportMeta",
  "atob",
  "btoa",
  "fetch",
]);
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
          const typeofQuerys = decl.getDescendantsOfKind(
            ts.SyntaxKind.TypeQuery,
          );
          if (
            typeofQuerys.some((q) => q.getExprName().getText() === "globalThis")
          ) {
            bannableNames.add(decl.getName());
          }
          names.add(decl.getName());
        }
      } else if (Node.hasName(statement)) {
        if (ignoredNames.has(statement.getName())) continue;
        names.add(statement.getName());
      }
    }
  }
}

console.log("Bannable names: ", Array.from(bannableNames).sort());
console.log(
  "Node only names: ",
  Array.from(names.difference(bannableNames)).sort(),
);
