// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";

export class One2ManyMap<KeyType, ValueType> {
  private map = new Map<KeyType, ValueType[]>();

  add(key: KeyType, value: ValueType) {
    if (!this.map.has(key)){
      this.map.set(key, []);
    }
    const array = this.map.get(key);
    array.push(value);
  }

  removeKey(key: KeyType) {
    this.map.delete(key);
  }

  has(key: KeyType) {
    return this.map.has(key);
  }

  forEach(key: KeyType, cb: (value: ValueType) => void) {
    if (this.has(key)) {
      const array = this.map.get(key);
      array.forEach(cb);
    }
  }
}

export function isNodeExported(node: ts.Node): boolean {
  return (ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Export) !== 0;
}

export function extractRefName(n: ts.EntityName): string {
  if (n.kind === ts.SyntaxKind.QualifiedName) {
    return extractRefName(n.left);
  }
  return n.text;
}

export function parseEntityName(source: ts.SourceFile, name: ts.EntityName) {
  const text = source.text.substring(name.pos, name.end);
  return {
    text: removeSpaces(text),
    refName: extractRefName(name)
  };
}

// https://www.ecma-international.org/ecma-262/6.0/#sec-white-space
const SPACES = [
  "\u0009",   // CHARACTER TABULATION
  "\u000b",   // LINE TABULATION
  "\u000c",   // FORM FEED (FF)
  "\u0020",   // SPACE
  "\u00A0",   // NO-BREAK SPACE
  "\uFEFF",   // ZERO WIDTH NO-BREAK SPACE
  // Zs: category
  "\u0020",   // SPACE
  "\u00A0",   // NO-BREAK SPACE
  "\u1680",   // OGHAM SPACE MARK
  "\u2000",   // EN QUAD
  "\u2001",   // EM QUAD
  "\u2002",   // EN SPACE
  "\u2003",   // EM SPACE
  "\u2004",   // THREE-PER-EM SPACE
  "\u2005",   // FOUR-PER-EM SPACE
  "\u2006",   // SIX-PER-EM SPACE
  "\u2007",   // FIGURE SPACE
  "\u2008",   // PUNCTUATION SPACE
  "\u2009",   // THIN SPACE
  "\u200A",   // HAIR SPACE
  "\u202F",   // NARROW NO-BREAK SPACE
  "\u205F",   // NARROW NO-BREAK SPACE
  "\u3000",   // IDEOGRAPHIC SPACE
];
  
export function isWhiteSpace(c: string) {
  return SPACES.indexOf(c) > -1;
}

export function removeSpaces(str: string) {
  let q = null;
  let ret = "";
  for (const c of str) {
    if (c === q) {
      q = null;
      ret += c;
      continue;
    }
    if (c === "\"" || c === "'" || c === "`") {
      q = c;
      ret += c;
      continue;
    }
    if (q) {
      ret += c;
      continue;
    }
    if (!isWhiteSpace(c)) {
      ret += c;
    }
  }
  return ret;
}
