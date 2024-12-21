// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

export interface NodeFacade {
  type: string;
  range: [number, number];
  [key: string]: unknown;
}

export interface AstContext {
  buf: Uint8Array;
  strTable: Map<number, string>;
  strTableOffset: number;
  rootOffset: number;
  nodes: Map<number, NodeFacade>;
  strByType: number[];
  strByProp: number[];
  typeByStr: Map<string, number>;
  propByStr: Map<string, number>;
  matcher: MatchContext;
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

export interface MatchContext {
  getFirstChild(id: number): number;
  getLastChild(id: number): number;
  getSiblings(id: number): number[];
  getParent(id: number): number;
  getType(id: number): number;
  hasAttrPath(id: number, propIds: number[], idx: number): boolean;
  getAttrPathValue(id: number, propIds: number[], idx: number): unknown;
}

export type NextFn = (ctx: MatchContext, id: number) => boolean;
export type MatcherFn = (ctx: MatchContext, id: number) => boolean;
export type TransformFn = (value: string) => number;
export type VisitorFn = (node: Deno.AstNode) => void;

export interface CompiledVisitor {
  matcher: (offset: number) => boolean;
  info: { enter: VisitorFn; exit: VisitorFn };
}

export {};
