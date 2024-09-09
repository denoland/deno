// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// This script is used to generate the @types/deno package on DefinitelyTyped.

import $ from "jsr:@david/dax@0.41.0";
import { Node, Project } from "jsr:@ts-morph/ts-morph@23.0.0";

const rootDir = $.path(import.meta.dirname!).parentOrThrow();
const definitelyTypedDir = rootDir.join(
  "../DefinitelyTyped/types/deno/",
);

if (!definitelyTypedDir.existsSync()) {
  throw new Error(`Makes sure ${definitelyTypedDir} exists.`);
}

const denoExec = rootDir.join(
  "target/debug/deno" + (Deno.build.os === "windows" ? ".exe" : ""),
);

await $`cargo build`;

await createDenoDtsFile();
await updatePkgJson();

async function createDenoDtsFile() {
  function matchesAny(text: string | undefined, patterns: string[]): boolean {
    if (text == null) {
      return false;
    }
    for (const pattern of patterns) {
      if (text.includes(pattern)) {
        return true;
      }
    }
    return false;
  }

  const text = await $`${denoExec} types`.text();
  const project = new Project();
  const file = project.createSourceFile(
    definitelyTypedDir.join("index.d.ts").toString(),
    text,
    {
      overwrite: true,
    },
  );

  for (const statement of file.getStatementsWithComments()) {
    if (Node.isCommentStatement(statement)) {
      const statementText = statement.getText();
      if (statementText.includes("<reference")) {
        statement.remove();
        continue;
      }
    }
    const shouldKeepKeep = (Node.isModuleDeclaration(statement) ||
      Node.isInterfaceDeclaration(statement) ||
      Node.isTypeAliasDeclaration(statement) ||
      Node.isClassDeclaration(statement)) &&
      (matchesAny(statement.getName(), [
        "Deno",
      ]) || statement.getName()?.startsWith("GPU"));
    if (!shouldKeepKeep) {
      statement.remove();
    }
  }

  file.insertStatements(
    0,
    '// Copyright 2018-2024 the Deno authors. MIT license.\n\n/// <reference lib="dom" />\n\n',
  );

  file.saveSync();
}

async function updatePkgJson() {
  const pkgJsonFile = definitelyTypedDir.join("package.json");
  const obj = pkgJsonFile.readJsonSync();
  // deno-lint-ignore no-explicit-any
  (obj as any).version = await getDenoVersion();
  pkgJsonFile.writeTextSync(JSON.stringify(obj, undefined, 4) + "\n"); // 4 spaces indent
}

async function getDenoVersion() {
  const text = await $`${denoExec} -v`.text();
  return text.match(/deno (.*)/)![1];
}
