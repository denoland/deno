// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import { define } from "../core";

// tslint:disable:only-arrow-functions

// Keyword Type Nodes
define("AnyKeyword", function(e) {
  e.push({ type: "any" });
});

define("UnknownKeyword", function(e) {
  e.push({ type: "<unknown>" });
});

define("NumberKeyword", function(e) {
  e.push({ type: "number" });
});

define("ObjectKeyword", function(e) {
  e.push({ type: "object" });
});

define("BooleanKeyword", function(e) {
  e.push({ type: "boolean" });
});

define("StringKeyword", function(e) {
  e.push({ type: "string" });
});

define("SymbolKeyword", function(e) {
  e.push({ type: "symbol" });
});

define("ThisKeyword", function(e) {
  e.push({ type: "this" });
});

define("VoidKeyword", function(e) {
  e.push({ type: "void" });
});

define("UndefinedKeyword", function(e) {
  e.push({ type: "undefined" });
});

define("NullKeyword", function(e) {
  e.push({ type: "null" });
});

define("NeverKeyword", function(e) {
  e.push({ type: "never" });
});

define("TrueKeyword", function(e) {
  e.push({ type: "true" });
});

define("FalseKeyword", function(e) {
  e.push({ type: "false" });
});
