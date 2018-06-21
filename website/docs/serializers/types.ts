// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { define, visit } from "../core";
import { parseEntityName } from "../util";

// tslint:disable:only-arrow-functions

define("TypeReference", function(e, node: ts.TypeReferenceNode) {
  const name = parseEntityName(this.sourceFile, node.typeName);
  const doc = {
    type: "TypeReference",
    name: name.text
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

// TypePredicate,
// TypeReference,
// FunctionType,
// ConstructorType,
// TypeQuery,
// TypeLiteral,
// ArrayType,
// TupleType,
// UnionType,
// IntersectionType,
// ConditionalType,
// InferType,
// ParenthesizedType,
// ThisType,
// TypeOperator,
// IndexedAccessType,
// MappedType,
// LiteralType,
