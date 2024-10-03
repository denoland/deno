#!/usr/bin/env -S deno run -A
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// This script is used to generate the @types/deno package on DefinitelyTyped.

import $ from "jsr:@david/dax@0.42.0";
import { Node, Project } from "jsr:@ts-morph/ts-morph@23.0.0";
import * as semver from "jsr:@std/semver@1.0.3";

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

$.logStep("Building Deno executable...");
await $`cargo build`;

$.logStep("Creating declaration file...");
await createDenoDtsFile();
$.logStep("Updating package.json...");
await updatePkgJson();
$.logStep("Formatting...");
await $`pnpm dprint fmt`.cwd(definitelyTypedDir);

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
      continue;
    }
  }

  file.insertStatements(
    0,
    "// Copyright 2018-2024 the Deno authors. MIT license.\n\n",
  );

  file.saveSync();
}

async function updatePkgJson() {
  const pkgJsonFile = definitelyTypedDir.join("package.json");
  const obj = pkgJsonFile.readJsonSync();
  const version = semver.parse(await getDenoVersion());
  version.patch = 9999;
  version.prerelease = undefined;
  version.build = undefined;
  // deno-lint-ignore no-explicit-any
  (obj as any).version = semver.format(version);
  pkgJsonFile.writeTextSync(JSON.stringify(obj, undefined, 4) + "\n"); // 4 spaces indent
}

async function getDenoVersion() {
  const text = await $`${denoExec} -v`.text();
  return text.match(/deno (.*)/)![1];
}
