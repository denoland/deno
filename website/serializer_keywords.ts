// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import { VISITOR } from "./core";

// tslint:disable:only-arrow-functions

// Keyword Type Nodes
VISITOR("AnyKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "any"
  });
});

VISITOR("UnknownKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "<unknown>"
  });
});

VISITOR("NumberKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "number"
  });
});

VISITOR("ObjectKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "object"
  });
});

VISITOR("BooleanKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "boolean"
  });
});

VISITOR("StringKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "string"
  });
});

VISITOR("SymbolKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "symbol"
  });
});

VISITOR("ThisKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "this"
  });
});

VISITOR("VoidKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "void"
  });
});

VISITOR("UnVISITORdKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "unVISITORd"
  });
});

VISITOR("NullKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "null"
  });
});

VISITOR("NeverKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "never"
  });
});

VISITOR("TrueKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "true"
  });
});

VISITOR("FalseKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "false"
  });
});

VISITOR("UndefinedKeyword", function(e) {
  e.push({
    type: "keyword",
    name: "undefined"
  });
});
