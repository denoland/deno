// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { blue, red, yellow } from "../fmt/colors.ts";
import { walk } from "../fs/walk.ts";
import ts from "npm:typescript";
const {
  createSourceFile,
  ScriptTarget,
  SyntaxKind,
} = ts;

const EXTENSIONS = [".mjs", ".js", ".ts", ".md"];
const EXCLUDED_PATHS = [
  ".git",
  ".github",
  "_tools",
];

const ROOT = new URL("../", import.meta.url);
const ROOT_LENGTH = ROOT.pathname.slice(0, -1).length;

const RX_JSDOC_COMMENT = /\*\*[^*]*\*+(?:[^/*][^*]*\*+)*/gm;
const RX_JSDOC_REMOVE_LEADING_ASTERISK = /^\s*\* ?/gm;
const RX_CODE_BLOCK = /`{3}([\w]*)\n([\S\s]+?)\n`{3}/gm;

let shouldFail = false;
let countChecked = 0;

function checkImportStatements(
  codeBlock: string,
  filePath: string,
  lineNumber: number,
): void {
  const sourceFile = createSourceFile(
    "doc-imports-checker$",
    codeBlock,
    ScriptTarget.Latest,
  );
  const importDeclarations = sourceFile.statements.filter((s) =>
    s.kind === SyntaxKind.ImportDeclaration
  ) as ts.ImportDeclaration[];

  for (const importDeclaration of importDeclarations) {
    const { moduleSpecifier } = importDeclaration;
    const importPath = (moduleSpecifier as ts.StringLiteral).text;
    const isRelative = importPath.startsWith(".");
    const isInternal = importPath.startsWith(
      "https://deno.land/std@$STD_VERSION/",
    );
    const { line } = sourceFile.getLineAndCharacterOfPosition(
      moduleSpecifier.pos,
    );

    if (isRelative || !isInternal) {
      console.log(
        yellow("Warn ") +
          (isRelative
            ? "relative import path"
            : "external or incorrectly versioned dependency") +
          ": " +
          red(`"${importPath}"`) + " at " +
          blue(
            filePath.substring(ROOT_LENGTH + 1),
          ) + yellow(":" + (lineNumber + line)),
      );

      shouldFail = true;
    }
  }
}

function checkCodeBlocks(
  content: string,
  filePath: string,
  lineNumber = 1,
): void {
  for (const codeBlockMatch of content.matchAll(RX_CODE_BLOCK)) {
    const [, language, codeBlock] = codeBlockMatch;
    const codeBlockLineNumber =
      content.slice(0, codeBlockMatch.index).split("\n").length;

    if (
      ["ts", "js", "typescript", "javascript", ""].includes(
        language.toLocaleLowerCase(),
      )
    ) {
      checkImportStatements(
        codeBlock,
        filePath,
        lineNumber + codeBlockLineNumber,
      );
    }
  }
}

for await (
  const { path } of walk(ROOT, {
    exts: EXTENSIONS,
    includeDirs: false,
    skip: EXCLUDED_PATHS.map((p) => new RegExp(p + "$")),
  })
) {
  const content = await Deno.readTextFile(path);
  countChecked++;

  if (path.endsWith(".md")) {
    checkCodeBlocks(content, path);
  } else {
    for (const jsdocMatch of content.matchAll(RX_JSDOC_COMMENT)) {
      const comment = jsdocMatch[0].replaceAll(
        RX_JSDOC_REMOVE_LEADING_ASTERISK,
        "",
      );
      const commentLineNumber =
        content.slice(0, jsdocMatch.index).split("\n").length;

      checkCodeBlocks(comment, path, commentLineNumber);
    }
  }
}

console.log(`Checked ${countChecked} files`);
if (shouldFail) Deno.exit(1);
