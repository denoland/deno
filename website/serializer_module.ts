// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./core";
import { isNodeExported } from "./util";

// tslint:disable:only-arrow-functions

VISITOR("ModuleDeclaration", function(e, node: ts.ModuleDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  console.log("ModuleDeclaration", node);
  const array = [];
  visit.call(this, array, node.name);
  const name = array[0];
  array.length = 0;
  visit.call(this, array, node.body);
  e.push({
    type: "module",
    documentation: ts.displayPartsToString(docs),
    name,
    statements: array
  });
});

VISITOR("ModuleBlock", function(e, node: ts.ModuleBlock) {
  if (!node.statements) return;
  const array = [];
  // Only visit exported declarations in first round.
  for (let i = node.statements.length - 1;i >= 0;--i) {
    if (isNodeExported(node.statements[i])) {
      visit.call(this, array, node.statements[i]);
    }
  }
  array.reverse();
  e.push(...array);
  // TODO visit while this.privateNames is not empty
});

VISITOR("SourceFile", "ModuleBlock");
