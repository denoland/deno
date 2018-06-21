// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { define, visit } from "../core";
import { parseEntityName } from "../util";

// tslint:disable:only-arrow-functions

define("FunctionDeclaration", function(e, node: ts.FunctionDeclaration) {
  // Get signature of node so we can extract it's documentations comment.
  const sig = this.checker.getSignatureFromDeclaration(node);
  const docs = sig.getDocumentationComment(this.checker);

  // Serialize parameters.
  const parameters = [];
  for (const param of node.parameters) {
    visit.call(this, parameters, param);
  }

  // Get return type
  const returnTypes = [];
  if (node.type) {
    visit.call(this, returnTypes, node.type);
  }

  const typeParameters = [];
  if (node.typeParameters) {
    for (const t of node.typeParameters) {
      visit.call(this, typeParameters, t);
    }
  }

  // TODO
  // As we serialized parameters it means we might have some types in
  // this.privateNames which are actually defined in node.parameterTypes
  // we must remove those objects from this.privateNames.

  e.push({
    type: "function",
    name: node.name.text,
    documentation: ts.displayPartsToString(docs),
    parameters,
    returnType: returnTypes[0],
    typeParameters,
    generator: !!node.asteriskToken
  });
});

define("Parameter", function(e, node: ts.ParameterDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);

  const types = [];
  visit.call(this, types, node.type);

  e.push({
    name: symbol.getName(),
    type: types[0],
    documentation: ts.displayPartsToString(docs),
    optional: !!node.questionToken
  });
});

define("TypeParameter", function(e, node: ts.TypeParameterDeclaration) {
  // constraint
  const constraints = [];
  if (node.constraint) {
    visit.call(this, constraints, node.constraint);
  }
  const name = parseEntityName(this.sourceFile, node.name);
  e.push({
    name: name.text,
    constraint: constraints[0]
  });
});
