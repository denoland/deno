// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { defineVisitor, visit } from "./parser";
import { setFilename } from "./util";

// tslint:disable:only-arrow-functions
// tslint:disable:object-literal-sort-keys

defineVisitor("VariableStatement", function(e, node: ts.VariableStatement) {
  visit.call(this, e, node.declarationList);
});

defineVisitor("VariableDeclarationList", function(
  e,
  node: ts.VariableDeclarationList
) {
  const declarations = [];
  for (const d of node.declarations) {
    visit.call(this, declarations, d);
  }

  const symbol = this.checker.getSymbolAtLocation(node.declarations[0].name);
  const docs = symbol.getDocumentationComment(this.checker);

  e.push({
    type: "VariableDeclarationList",
    isConstant: (node.flags & ts.NodeFlags.Const) !== 0,
    documentation: ts.displayPartsToString(docs),
    declarations
  });
});

defineVisitor("VariableDeclaration", function(e, node: ts.VariableDeclaration) {
  const array = [];
  visit.call(this, array, node.name);
  const name = array[0];
  array.length = 0;
  visit.call(this, array, node.initializer);
  let initializer = array[0];
  if (!initializer && node.initializer) {
    initializer = {
      type: "value",
      text: "..."
    };
  }
  array.length = 0;
  visit.call(this, array, node.type);
  const dataType = array[0];
  e.push({
    type: "VariableDeclaration",
    name: name.text,
    initializer,
    dataType
  });
  setFilename(this, name.refName);
});

defineVisitor("ArrayLiteralExpression", function(e) {
  e.push({
    type: "value",
    text: "[...]"
  });
});

defineVisitor("CallExpression", function(e, node: ts.CallExpression) {
  let text = "[FUNCTION CALL]";
  if (ts.isIdentifier(node.expression)) {
    text = `${node.expression.text}(...)`;
  }
  e.push({
    type: "value",
    text
  });
});

defineVisitor("NewExpression", function(e, node: ts.NewExpression) {
  let text = "new ?(...)";
  if (ts.isIdentifier(node.expression)) {
    text = `new ${node.expression.text}(...)`;
  }
  e.push({
    type: "value",
    text
  });
});

// Note: we don't need to implement all of the possible initialize values.
// like: BinaryExpression
