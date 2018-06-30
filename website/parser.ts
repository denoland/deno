// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as fs from "fs";
import * as ts from "typescript";
import * as types from "./types";
import { One2ManyMap } from "./util";

// We would have lots of `if (...) return;` in this code.
// so let's turn this tslint rule off.
// tslint:disable:curly

const VISITORS = new Map<string, types.Visitor | string>();

/**
 * Defines a visitor which will be used later in visit function.
 * @internal
 */
export function VISITOR(name: string, visitor: types.Visitor | string): void {
  VISITORS.set(name, visitor);
}

/**
 * Call a visitor defined via `define()` based on given node's kind.
 * It can also be used to serialize a node.
 * @internal
 */
export function visit(this: types.TSKit, docEntries: any[], node: ts.Node,
  alias = false) {
  if (!node) return;
  // tslint:disable-next-line:no-any
  let kind = (ts as any).SyntaxKind[node.kind];
  if (alias) kind = alias;
  if (!VISITORS.has(kind)){
    console.log("[%s] Not defined.", kind, node);
    return;
  }
  const len = docEntries.length;
  // We don't return any value from this function
  // So whenever we need to get value from a visitor (1) we can just pass an
  // empty array to it and get our result from there
  //   const ret = [];
  //   visit(this, ret, node);
  //   ret[i];
  //
  // 1. We might want to do it because visitors are serializer functions.
  const cb = VISITORS.get(kind);
  if (typeof cb === "string") {
    visit.call(this, docEntries, node, cb);
  } else {
    cb.call(this, docEntries, node);
  }
  if (docEntries.length === len) {
    console.log("[%s] Empty return.", kind);
  }
}

/**
 * Extract documentation from source code.
 */
export function generateDoc(fileName: string, options: ts.CompilerOptions) {
  const s = fs.readFileSync(fileName).toString();
  const host = createCompilerHost(options, s)
  const program = ts.createProgram(["file.ts"], options, host);
  const checker = program.getTypeChecker();
  let sourceFile;
  for (const s of program.getSourceFiles()) {
    if (s.fileName === "file.ts") {
      sourceFile = s;
      break;
    }
  }
  const kit: types.TSKit = {
    sourceFile,
    checker,
    privateNames: new One2ManyMap(),
    typeParameters: [],
    currentNamespace: [],
    isJS: fileName.endsWith(".js")
  };
  const docEntries = [];
  visit.call(kit, docEntries, sourceFile);
  return docEntries;
}

// TODO(qti3e) Needs to be rewritten.
function createCompilerHost(options: ts.CompilerOptions, sourceCode: string)
  : ts.CompilerHost {
  return {
    getSourceFile,
    getDefaultLibFileName: () => "",
    writeFile: nop,
    getCurrentDirectory: nop,
    getDirectories: nop,
    getCanonicalFileName: fileName => fileName,
    getNewLine: () => ts.sys.newLine,
    useCaseSensitiveFileNames: () => true,
    fileExists,
    readFile,
    resolveModuleNames
  }

  function nop(): any {}

  function fileExists(fileName: string): boolean {
    return fileName === "file.ts";
  }

  function readFile(fileName: string): string | undefined {
    if (fileName !== "file.ts") {
      throw new Error("File does not exsit.")
    }
    return sourceCode;
  }

  function getSourceFile(fileName: string, languageVersion: ts.ScriptTarget,
    onError?: (message: string) => void) {
    const sourceText = readFile(fileName);
    return sourceText !== undefined ?
      ts.createSourceFile(fileName, sourceText, languageVersion) : undefined;
  }

  function resolveModuleNames(moduleNames: string[], containingFile: string)
      : ts.ResolvedModule[] {
    const resolvedModules: ts.ResolvedModule[] = [];
    for (const _ of moduleNames) {
      resolvedModules.push(null);
    }
    return resolvedModules;
  }
}

// Import serializers.
import "./serializer_function";
import "./serializer_types";
import "./serializer_keywords";
import "./serializer_interface";
import "./serializer_enum";
import "./serializer_class";
import "./serializer_object";
import "./serializer_declaration";
import "./serializer_module";
