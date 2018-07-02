// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";

export class CompilerHost implements ts.CompilerHost {
  constructor(private sourceText: string) {}

  getSourceFile(
    fileName: string,
    languageVersion: ts.ScriptTarget,
    onError?: (msg: string) => void
  ) {
    const sourceText = this.readFile(fileName);
    if (sourceText === undefined) return undefined;
    return ts.createSourceFile(fileName, sourceText, languageVersion);
  }

  readFile(fileName: string): string {
    if (fileName === "deno.ts") {
      return this.sourceText;
    }
    return undefined;
  }

  fileExists(fileName: string): boolean {
    return fileName === "deno.ts";
  }

  resolveModuleNames(moduleNames: string[]) {
    return new Array(moduleNames.length).fill(null);
  }

  writeFile() {}
  getDefaultLibFileName() {
    return "";
  }
  getCurrentDirectory() {
    return "";
  }
  getDirectories() {
    return [];
  }
  getCanonicalFileName(f) {
    return f;
  }
  getNewLine() {
    return "\n";
  }
  useCaseSensitiveFileNames() {
    return true;
  }
}
