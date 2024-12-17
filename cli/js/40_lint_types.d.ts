// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

export interface AstContext {
  buf: Uint8Array;
  strTable: Map<number, string>;
  idTable: number[];
  rootId: number;
  stack: number[];
}

export interface LintState {
  plugins: Deno.LintPlugin[];
  installedPlugins: Set<string>;
}

export interface AttrExists {
  type: 3;
  prop: number[];
}

export interface AttrBin {
  type: 4;
  prop: number[];
  op: number;
  value: string;
}

export type AttrSelector = AttrExists | AttrBin;

export interface ElemSelector {
  type: 1;
  wildcard: boolean;
  elem: number;
}

export interface PseudoNthChild {
  type: 5;
  op: string | null;
  step: number;
  stepOffset: number;
  of: Selector | null;
  repeat: boolean;
}

export interface PseudoHas {
  type: 6;
  selectors: Selector[];
}
export interface PseudoNot {
  type: 7;
  selectors: Selector[];
}
export interface PseudoFirstChild {
  type: 8;
}
export interface PseudoLastChild {
  type: 9;
}

export interface Relation {
  type: 2;
  op: number;
}

export type Selector = Array<
  | ElemSelector
  | Relation
  | AttrExists
  | AttrBin
  | PseudoNthChild
  | PseudoNot
  | PseudoHas
  | PseudoFirstChild
  | PseudoLastChild
>;

export interface SelectorParseCtx {
  root: Selector;
  current: Selector;
}

export const enum SelToken {
  Value,
  Char,
  EOF,
}

export interface ILexer {
  token: SelToken;
}

export interface MatchCtx {
  getFirstChild(id: number): number;
  getLastChild(id: number): number;
  getSiblingBefore(parentId: number, sib: number): number;
  getSiblings(id: number): number[];
  getParent(id: number): number;
  getType(id: number): number;
  hasAttrPath(id: number, propIds: number[]): boolean;
  getAttrPathValue(id: number, propIds: number[]): unknown;
}

export type NextFn = (ctx: MatchCtx, id: number) => boolean;
export type MatcherFn = (ctx: MatchCtx, id: number) => boolean;
export type TransformFn = (value: string) => number;
export type VisitorFn = (node: Deno.AstNode) => void;

export interface CompiledVisitor {
  matcher: MatcherFn;
  info: { enter: VisitorFn; exit: VisitorFn };
}

export {};
