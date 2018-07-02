// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { visit, VISITOR } from "./parser";
import { setFilename } from "./util";

// tslint:disable:only-arrow-functions
// tslint:disable:object-literal-sort-keys

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
  const modifierFlags = ts.getCombinedModifierFlags(node);
  const isDefault = (modifierFlags & ts.ModifierFlags.Default) !== 0;
  e.push({
    type: "interface",
    name: node.name.text,
    documentation: ts.displayPartsToString(docs),
    parameters,
    heritageClauses,
    members,
    isDefault
  });
  this.typeParameters.splice(len);
  setFilename(this, node.name.text);
});

VISITOR("ExpressionWithTypeArguments", function(
  e,
  node: ts.ExpressionWithTypeArguments
) {
  const expressions = [];
  visit.call(this, expressions, node.expression);
  const expression = expressions[0];
  const typeArguments = [];
  if (node.typeArguments) {
    for (const t of node.typeArguments) {
      visit.call(this, typeArguments, t);
    }
  }
  const doc = {
    type: "ExpressionWithTypeArguments",
    expression: expression.text,
    arguments: typeArguments
  };
  e.push(doc);
  this.privateNames.add(expression.refName, doc);
});

VISITOR("Identifier", function(e, node: ts.Identifier) {
  if (node.text === "undefined") {
    e.push({
      type: "keyword",
      name: "undefined"
    });
    return;
  }
  e.push({
    type: "name",
    text: node.text,
    refName: node.text
  });
});

VISITOR("MethodSignature", function(e, node: ts.MethodSignature) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol && symbol.getDocumentationComment(this.checker);

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
    documentation: ts.displayPartsToString(docs),
    name: names[0].text,
    parameters,
    dataType: types[0],
    optional: !!node.questionToken
  });
});
