// Copyright 2018-2025 the Deno authors. MIT license.

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

// TODO(@marvinhagemeister) Remove once we land "official" types
export interface RuleContext {
  id: string;
}

// TODO(@marvinhagemeister) Remove once we land "official" types
export interface LintRule {
  create(ctx: RuleContext): Record<string, (node: unknown) => void>;
  destroy?(ctx: RuleContext): void;
}

// TODO(@marvinhagemeister) Remove once we land "official" types
export interface LintPlugin {
  name: string;
  rules: Record<string, LintRule>;
}

export interface LintState {
  plugins: LintPlugin[];
  installedPlugins: Set<string>;
}

export type VisitorFn = (node: unknown) => void;

export interface CompiledVisitor {
  matcher: (ctx: MatchContext, offset: number) => boolean;
  info: { enter: VisitorFn; exit: VisitorFn };
}

export interface AttrExists {
  type: 3;
  prop: number[];
}

export interface AttrBin {
  type: 4;
  prop: number[];
  op: number;
  // deno-lint-ignore no-explicit-any
  value: any;
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

export {};
