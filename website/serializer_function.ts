// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./core";
import { getModifiers } from "./util";

// tslint:disable:only-arrow-functions

VISITOR("FunctionDeclaration", function(e, node: ts.FunctionDeclaration) {
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
  if (returnTypes.length === 0) {
    returnTypes.push({});
  }
  // Return documentation
  const retDocs = sig.getJsDocTags().filter(({ name }) => name === "return");
  returnTypes[0].documentation = retDocs.map(({ text }) => text).join(" ");

  const typeParameters = [];
  const len = this.typeParameters.length;
  if (node.typeParameters) {
    for (const t of node.typeParameters) {
      visit.call(this, typeParameters, t);
    }
  }

  e.push({
    type: "function",
    name: node.name && node.name.text,
    documentation: ts.displayPartsToString(docs),
    parameters,
    returnType: returnTypes[0],
    typeParameters,
    generator: !!node.asteriskToken
  });
  this.typeParameters.splice(len);
});

// Alias
VISITOR("FunctionExpression", "FunctionDeclaration");

VISITOR("Parameter", function(e, node: ts.ParameterDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);

  const types = [];
  visit.call(this, types, node.type);

  const data = {
    name: symbol.getName(),
    type: types[0],
    documentation: ts.displayPartsToString(docs),
    optional: !!node.questionToken,
  };

  // If parent is a class constructor, include node's modifiers.
  if (node.parent.kind === ts.SyntaxKind.Constructor) {
    Object.assign(data, getModifiers(node));
  }

  e.push(data);
});

VISITOR("ArrowFunction", function(e, node: ts.ArrowFunction) {
  const sig = this.checker.getSignatureFromDeclaration(node);
  const docs = sig.getDocumentationComment(this.checker);

  const array = [];
  visit.call(this, array, node.type);
  const returnType = array[0];
  const parameters = [];
  if (node.parameters) {
    for (const p of node.parameters) {
      visit.call(this, parameters, p);
    }
  }

  const typeParameters = [];
  const len = this.typeParameters.length;
  if (node.typeParameters) {
    for (const t of node.typeParameters) {
      visit.call(this, typeParameters, t);
    }
  }

  e.push({
    type: "ArrowFunction",
    documentation: ts.displayPartsToString(docs),
    returnType,
    parameters,
    typeParameters
  });
  this.typeParameters.splice(len);
});
