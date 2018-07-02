// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import { defineVisitor } from "./parser";

// tslint:disable:only-arrow-functions
// tslint:disable:object-literal-sort-keys

// Keyword Type Nodes
defineVisitor("AnyKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "any"
  });
});

defineVisitor("UnknownKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "<unknown>"
  });
});

defineVisitor("NumberKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "number"
  });
});

defineVisitor("ObjectKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "object"
  });
});

defineVisitor("BooleanKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "boolean"
  });
});

defineVisitor("StringKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "string"
  });
});

defineVisitor("SymbolKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "symbol"
  });
});

defineVisitor("ThisKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "this"
  });
});

defineVisitor("VoidKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "void"
  });
});

defineVisitor("UnVISITORdKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "undefineVisitord"
  });
});

defineVisitor("NullKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "null"
  });
});

defineVisitor("NeverKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "never"
  });
});

defineVisitor("TrueKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "true"
  });
});

defineVisitor("FalseKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "false"
  });
});

defineVisitor("UndefinedKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "undefined"
  });
});
