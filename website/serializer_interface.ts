// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./core";
import { setFilename } from "./util";

// tslint:disable:only-arrow-functions

VISITOR("InterfaceDeclaration", function(e, node: ts.InterfaceDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const heritageClauses = [];
  if (node.heritageClauses) {
    for (const c of node.heritageClauses) {
      for (const t of c.types) {
        visit.call(this, heritageClauses, t);
      }
    }
  }
  const parameters = [];
  const len = this.typeParameters.length;
  if (node.typeParameters) {
    for (const t of node.typeParameters) {
      visit.call(this, parameters, t);
    }
  }
  const members = [];
  if (node.members) {
    for (const t of node.members) {
      visit.call(this, members, t);
    }
  }
  e.push({
    type: "interface",
    name: node.name.text,
    documentation: ts.displayPartsToString(docs),
    parameters,
    heritageClauses,
    members
  });
  this.typeParameters.splice(len);
  setFilename(this, node.name.text);
});

VISITOR("ExpressionWithTypeArguments",
  function(e, node: ts.ExpressionWithTypeArguments) {
  const expressions = [];
  visit.call(this, expressions, node.expression);
  const typeArguments = [];
  if (node.typeArguments) {
    for (const t of node.typeArguments) {
      visit.call(this, typeArguments, t);
    }
  }
  e.push({
    type: "ExpressionWithTypeArguments",
    expression: expressions[0],
    arguments: typeArguments
  });
});

VISITOR("Identifier", function(e, node: ts.Identifier) {
  e.push(node.text);
});

VISITOR("MethodSignature", function(e, node: ts.MethodSignature) {
  const names = [];
  visit.call(this, names, node.name);
  const parameters = [];
  if (node.parameters) {
    for (const t of node.parameters) {
      visit.call(this, parameters, t);
    }
  }
  const types = [];
  visit.call(this, types, node.type);
  e.push({
    type: "MethodSignature",
    name: names[0],
    parameters,
    dataType: types[0],
    optional: !!node.questionToken
  });
});
