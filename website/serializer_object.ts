// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./parser";

// tslint:disable:only-arrow-functions

VISITOR("ObjectLiteralExpression",
  function(e, node: ts.ObjectLiteralExpression) {
  const properties = [];
  for (const p of node.properties) {
    visit.call(this, properties, p);
  }
  e.push({
    type: "ObjectLiteralExpression",
    properties
  });
});

VISITOR("PropertyAssignment", function(e, node: ts.PropertyAssignment) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const array = [];
  visit.call(this, array, node.name);
  const name = array[0];
  array.length = 0;
  visit.call(this, array, node.initializer);
  const initializer = array[0];
  e.push({
    type: "PropertyAssignment",
    documentation: ts.displayPartsToString(docs),
    name: name.text,
    initializer
  });
});

VISITOR("ShorthandPropertyAssignment",
  function(e, node: ts.ShorthandPropertyAssignment) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const docEntity = {
    type: "ShorthandPropertyAssignment",
    documentation: ts.displayPartsToString(docs),
    name: node.name.text
  };
  // Search for declaration.
  this.privateNames.add(docEntity.name, docEntity);
  e.push(docEntity);
});

VISITOR("SpreadAssignment", function(e, node: ts.SpreadAssignment) {
  // TODO(qti3e) Get documentation.
  const expressions = [];
  visit.call(this, expressions, node.expression);
  const docEntity = {
    type: "SpreadAssignment",
    expression: expressions[0]
  };
  // Search for declaration.
  if (ts.isIdentifier(node.expression)) {
    this.privateNames.add(node.expression.text, docEntity);
  }
  e.push(docEntity);
});
