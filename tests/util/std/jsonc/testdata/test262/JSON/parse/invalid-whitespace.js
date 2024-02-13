// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
esid: sec-json.parse
es5id: 15.12.1.1-0-7
description: >
    other category z spaces are not valid JSON whitespace as specified
    by the production JSONWhitespace.
---*/

assert.throws(SyntaxError, function () {
  JSON.parse("\u16801");
}, "\\u1680");

assert.throws(SyntaxError, function () {
  JSON.parse("\u180e1");
}, "\\u180e");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20001");
}, "\\u2000");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20011");
}, "\\u2001");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20021");
}, "\\u2002");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20031");
}, "\\u2003");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20041");
}, "\\u2004");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20051");
}, "\\u2005");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20061");
}, "\\u2006");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20071");
}, "\\u2007");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20081");
}, "\\u2008");

assert.throws(SyntaxError, function () {
  JSON.parse("\u20091");
}, "\\u2009");

assert.throws(SyntaxError, function () {
  JSON.parse("\u200a1");
}, "\\u200a");

assert.throws(SyntaxError, function () {
  JSON.parse("\u202f1");
}, "\\u202f");

assert.throws(SyntaxError, function () {
  JSON.parse("\u205f1");
}, "\\u205f");

assert.throws(SyntaxError, function () {
  JSON.parse("\u30001");
}, "\\u3000");
