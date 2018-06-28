// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import * as types from "./types";

const mapSeparator = Symbol();

export class One2ManyMap<KeyType, ValueType> {
  private data = new Map<KeyType, (ValueType | typeof mapSeparator)[]>();
  private locked = false;
  public changed = false;

  lock(): void {
    this.locked = true;
  }

  unlock(): void{
    this.locked = false;
  }

  add(key: KeyType, value: ValueType): void {
    if (this.locked) return;
    if (!this.data.has(key)) {
      this.data.set(key, []);
    }
    this.data.get(key).push(value);
    this.changed = true;
  }

  addSeparator(): void {
    this.data.forEach(array => {
      array.push(mapSeparator);
    });
  }

  clearKeyAfterLastSeparator(key: KeyType): void{
    if (!this.data.has(key)) return;
    const array = this.data.get(key);
    let index = array.lastIndexOf(mapSeparator);
    array.splice(index + 1);
    if (array.length === 0) this.data.delete(key);
    this.changed = true;
  }

  forEachAfterLastSeparator(key: KeyType, cb: (v: ValueType) => void): void {
    if (!this.data.has(key)) return;
    const array = this.data.get(key);
    let i = array.length;
    while (true) {
      const data = array[--i];
      if (data === mapSeparator) break;
      cb(data);
      if (i == 0) break;
    }
  }

  removeLastSeparator(): void {
    this.data.forEach(array => {
      const index = array.lastIndexOf(mapSeparator);
      if (index < 0) return;
      array.splice(index, 1);
    });
  }

  has(key: KeyType): boolean {
    if (!this.data.has(key)) return false;
    const array = this.data.get(key);
    let index = array.lastIndexOf(mapSeparator);
    if (index < 0) return array.length > 0;
    return array.length - 1 > index;
  }

  isEmpty(): boolean {
    const keys = this.data.keys();
    for (const key of keys) {
      if (this.has(key)) {
        return false;
      }
    }
    return true;
  }
}

export function isNodeExported(node: ts.Node): boolean {
  return (ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Export) !== 0;
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
  
export function isWhiteSpace(c: string): boolean {
  return SPACES.indexOf(c) > -1;
}

export function removeSpaces(str: string): string {
  let q = null;
  let ret = "";
  let escaped = false;
  for (const c of str) {
    if (c === "\\") escaped = !escaped;
    if (c === "\"" || c === "'" || c === "`") {
      if (!escaped && q === c) {
        q = null;
      } else if (!escaped) {
        q = c;
      }
      ret += c;
    } else if (q || !(q || isWhiteSpace(c))) {
        ret += c;
    }
    if (c !== "\\") escaped = false;
  }
  return ret;
}

export interface NodeModifier {
  visibility?: "private" | "protected";
  isStatic?: boolean;
  isReadonly?: boolean;
}

export function getModifiers(node: ts.Node): NodeModifier {
  const ret: NodeModifier = {};
  const modifierFlags = ts.getCombinedModifierFlags(node);
  if ((modifierFlags & ts.ModifierFlags.Private) !== 0) {
    ret.visibility = "private";
  } else if ((modifierFlags & ts.ModifierFlags.Protected) !== 0) {
    ret.visibility = "protected";
  }
  if ((modifierFlags & ts.ModifierFlags.Static) !== 0) {
    ret.isStatic = true;
  }
  if ((modifierFlags & ts.ModifierFlags.Readonly) !== 0) {
    ret.isReadonly = true;
  }
  return ret;
}

export function setFilename(kit: types.TSKit, name: string, filename?: string)
  : void {
  if (!filename) {
    filename = "#" + kit.currentNamespace.join(".");
  }
  kit.privateNames.forEachAfterLastSeparator(name, e => {
    e.filename = filename;
  });
  kit.privateNames.clearKeyAfterLastSeparator(name);
}
