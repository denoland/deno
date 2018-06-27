// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

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
export function VISITOR(name: string, visitor: types.Visitor | string) {
  VISITORS.set(name, visitor);
}

/**
 * Call a visitor defined via `define()` based on given node's kind.
 * It can also be used to serialize a node.
 * @internal
 */
export function visit(this: types.TSKit, docEntries: any[], node: ts.Node, alias = false) {
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
  const program = ts.createProgram([fileName], options);
  const checker = program.getTypeChecker();
  let finalSourceFile;
  for (const sourceFile of program.getSourceFiles()) {
    // TODO Compare file names, user might want to see doc for declaration file.
    if (!sourceFile.isDeclarationFile) {
      finalSourceFile = sourceFile;
      break;
    }
  }
  if (!finalSourceFile) return null;
  const kit: types.TSKit = {
    sourceFile: finalSourceFile,
    checker,
    privateNames: new One2ManyMap(),
    typeParameters: [],
    currentNamespace: []
  };
  const docEntries = [];
  visit.call(kit, docEntries, finalSourceFile);
  return docEntries;
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
