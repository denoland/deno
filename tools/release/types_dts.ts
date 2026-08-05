// Copyright 2018-2026 the Deno authors. MIT license.

// Shared logic for turning the output of `deno types` into a `.d.ts` file that
// only declares the ambient `Deno` namespace (and the global `GPU*` WebGPU
// types it references). The other web platform globals emitted by `deno types`
// are stripped out because they would otherwise conflict with the standard
// TypeScript libs when the declaration file is used in a project.
//
// This is used both to generate the `@types/deno` package on DefinitelyTyped
// (see ../generate_types_deno.ts) and the `@deno/types` npm package (see
// ./npm/build.ts).

import { Node, Project } from "jsr:@ts-morph/ts-morph@27.0.0";

/** Transforms the output of `deno types` into the declaration file shipped in
 * the `@deno/types` and `@types/deno` packages. */
export function generateDenoTypesDts(denoTypesOutput: string): string {
  const project = new Project();
  const file = project.createSourceFile("deno.d.ts", denoTypesOutput, {
    overwrite: true,
  });

  function shouldKeepNode(node: Node): boolean {
    if (!Node.hasName(node)) {
      return false;
    }
    const name = node.getName();
    return name === "Deno" || name.startsWith("GPU");
  }

  for (const statement of file.getStatementsWithComments()) {
    if (Node.isCommentNode(statement)) {
      statement.remove();
      continue;
    }
    if (Node.isVariableStatement(statement)) {
      for (const decl of statement.getDeclarations()) {
        if (!shouldKeepNode(decl)) {
          decl.remove();
        }
      }
    } else if (!shouldKeepNode(statement)) {
      statement.remove();
    }
  }

  file.insertStatements(
    0,
    "// Copyright 2018-2026 the Deno authors. MIT license.\n\n",
  );

  return file.getFullText();
}
