// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import { VISITOR } from "./core";

// tslint:disable:only-arrow-functions

// Keyword Type Nodes
VISITOR("AnyKeyword", function(e) {
  e.push({ type: "any" });
});

VISITOR("UnknownKeyword", function(e) {
  e.push({ type: "<unknown>" });
});

VISITOR("NumberKeyword", function(e) {
  e.push({ type: "number" });
});

VISITOR("ObjectKeyword", function(e) {
  e.push({ type: "object" });
});

VISITOR("BooleanKeyword", function(e) {
  e.push({ type: "boolean" });
});

VISITOR("StringKeyword", function(e) {
  e.push({ type: "string" });
});

VISITOR("SymbolKeyword", function(e) {
  e.push({ type: "symbol" });
});

VISITOR("ThisKeyword", function(e) {
  e.push({ type: "this" });
});

VISITOR("VoidKeyword", function(e) {
  e.push({ type: "void" });
});

VISITOR("UnVISITORdKeyword", function(e) {
  e.push({ type: "unVISITORd" });
});

VISITOR("NullKeyword", function(e) {
  e.push({ type: "null" });
});

VISITOR("NeverKeyword", function(e) {
  e.push({ type: "never" });
});

VISITOR("TrueKeyword", function(e) {
  e.push({ type: "true" });
});

VISITOR("FalseKeyword", function(e) {
  e.push({ type: "false" });
});

VISITOR("UndefinedKeyword", function(e) {
  e.push({ type: "undefined" })
});
