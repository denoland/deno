// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { define, visit } from "../core";
import { parseEntityName, removeSpaces } from "../util";

// tslint:disable:only-arrow-functions

define("TypeAliasDeclaration", function(e, node: ts.TypeAliasDeclaration) {
  const name = parseEntityName(this.sourceFile, node.name);
  const types = [];
  visit.call(this, types, node.type);
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  e.push({
    type: "type",
    name: name.text,
    definition: types[0],
    documentation: ts.displayPartsToString(docs)
    // TODO type parameters
  });
  // TODO It's a definition so we should set definition source of
  // private names in `this.privateNames`
});

define("TypeReference", function(e, node: ts.TypeReferenceNode) {
  const name = parseEntityName(this.sourceFile, node.typeName);
  const doc = {
    type: "TypeReference",
    name: name.text,
    // TODO type arguments
    // This value will be filled in next iterations.
    // file: ?
  };
  e.push(doc);
  // Pushing to privateNames means we're looking for it's definition.
  this.privateNames.add(name.refName, doc);
});

define("UnionType", function(e, node: ts.UnionTypeNode) {
  const types = [];
  for (const t of node.types) {
    visit.call(this, types, t);
  }
  e.push({
    type: "UnionType",
    types
  });
});

define("IntersectionType", function(e, node: ts.IntersectionTypeNode) {
  const types = [];
  for (const t of node.types) {
    visit.call(this, types, t);
  }
  e.push({
    type: "IntersectionType",
    types
  });
});

define("LiteralType", function(e, node: ts.LiteralTypeNode) {
  visit.call(this, e, node.literal);
});

define("StringLiteral", function(e, node: ts.StringLiteral) {
  e.push({
    type: "string",
    text: node.text
  });
});

// TODO Need investigation.
define("FirstLiteralToken", function(e, node: ts.NumericLiteral) {
  e.push({
    type: "number",
    text: node.text
  });
});

define("ArrayType", function(e, node: ts.ArrayTypeNode) {
  const types = [];
  visit.call(this, types, node.elementType);
  e.push({
    type: "ArrayType",
    elementType: types[0]
  });
});

define("FunctionType", function(e, node: ts.FunctionTypeNode) {
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
    type: "FunctionType",
    parameters,
    returnType: returnTypes[0],
    typeParameters
  });
});

define("TupleType", function(e, node: ts.TupleTypeNode) {
  const types = [];
  for (const t of node.elementTypes) {
    visit.call(this, types, t);
  }
  e.push({
    type: "TupleType",
    elementTypes: types
  });
});

define("ParenthesizedType", function(e, node: ts.ParenthesizedTypeNode) {
  const types = [];
  visit.call(this, types, node.type);
  e.push({
    type: "ParenthesizedType",
    elementType: types[0]
  });
});

define("TypeLiteral", function(e, node: ts.TypeLiteralNode) {
  const members = [];
  for (const t of node.members) {
    visit.call(this, members, t);
  }
  e.push({
    type: "TypeLiteral",
    members
  });
});

define("IndexSignature", function(e, node: ts.IndexSignatureDeclaration) {
  const sig = this.checker.getSignatureFromDeclaration(node);
  const docs = sig.getDocumentationComment(this.checker);
  const parameters = [];
  for (const t of node.parameters) {
    visit.call(this, parameters, t);
  }
  const types = [];
  visit.call(this, types, node.type);
  e.push({
    type: "IndexSignature",
    parameters,
    returnType: types[0],
    documentation: ts.displayPartsToString(docs),
  });
});

define("ConstructSignature", function(e, n: ts.ConstructSignatureDeclaration) {
  const sig = this.checker.getSignatureFromDeclaration(n);
  const docs = sig.getDocumentationComment(this.checker);
  const parameters = [];
  for (const t of n.parameters) {
    visit.call(this, parameters, t);
  }
  const types = [];
  visit.call(this, types, n.type);
  e.push({
    type: "ConstructSignature",
    parameters,
    returnType: types[0],
    documentation: ts.displayPartsToString(docs),
  });
});

define("PropertySignature", function(e, node: ts.PropertySignature) {
  const types = [];
  visit.call(this, types, node.type);
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const names = [];
  if (ts.isIdentifier(node.name)) {
    names.push((node.name as ts.Identifier).text);
  } else {
    visit.call(this, names, node.name);
  }
  e.push({
    types: "PropertySignature",
    name: names[0],
    optional: !!node.questionToken,
    dataType: types[0],
    documentation: ts.displayPartsToString(docs),
  });
});

define("ComputedPropertyName", function(e, node: ts.ComputedPropertyName) {
  visit.call(this, e, node.expression);
});

define("PropertyAccessExpression",
  function(e, node: ts.PropertyAccessExpression) {
  const code = this.sourceFile.text.substring(node.pos, node.end);
  e.push(removeSpaces(code));
});

define("ConditionalType", function(e, node: ts.ConditionalTypeNode) {
  const array = [];
  visit.call(this, array, node.checkType);
  const checkType = array[0];
  array.length = 0;
  visit.call(this, array, node.extendsType);
  const extendsType = array[0];
  array.length = 0;
  visit.call(this, array, node.falseType);
  const falseType = array[0];
  array.length = 0;
  visit.call(this, array, node.trueType);
  const trueType = array[0];
  e.push({
    type: "ConditionalType",
    checkType,
    extendsType,
    falseType,
    trueType
  });
});

// TypeQuery,
// InferType,
// ThisType,
// TypeOperator,
// IndexedAccessType,
// MappedType,
