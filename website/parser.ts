// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as fs from "fs";
import * as ts from "typescript";
import { CompilerHost } from "./tshost";
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
  if (!VISITORS.has(kind)) return;
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
}

/**
 * Extract documentation from source code.
 */
export function generateDoc(fileName: string, options: ts.CompilerOptions) {
  const s = fs.readFileSync(fileName).toString();
  const host = new CompilerHost(s);
  const program = ts.createProgram(["deno.ts"], options, host);
  const checker = program.getTypeChecker();
  let sourceFile;
  for (const s of program.getSourceFiles()) {
    if (s.fileName === "deno.ts") {
      sourceFile = s;
      break;
    }
  }
  if (!sourceFile) return null;
  const kit: types.TSKit = {
    sourceFile,
    checker,
    privateNames: new One2ManyMap(),
    typeParameters: [],
    currentNamespace: [],
    isJS: fileName.endsWith(".js"),
    isDeclarationFile: fileName.endsWith(".d.ts")
  };
  const docEntries = [];
  visit.call(kit, docEntries, sourceFile);
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
