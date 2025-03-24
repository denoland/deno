// Copyright 2018-2025 the Deno authors. MIT license.

export interface AstContext {
  buf: Uint8Array;
  strTable: Map<number, string>;
  strTableOffset: number;
  rootOffset: number;
  nodes: Map<number, Deno.lint.Node>;
  spansOffset: number;
  propsOffset: number;
  strByType: number[];
  strByProp: number[];
  typeByStr: Map<string, number>;
  propByStr: Map<string, number>;
  matcher: MatchContext;
}

export interface LintState {
  plugins: Deno.lint.Plugin[];
  installedPlugins: Set<string>;
  /** format: `<plugin>/<rule>` */
  ignoredRules: Set<string>;
}

export type VisitorFn = (node: unknown) => void;

export interface CompiledVisitor {
  matcher: MatcherFn;
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

export interface FieldSelector {
  type: 10;
  props: number[];
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
export interface PseudoIs {
  type: 11;
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
  | FieldSelector
  | Relation
  | AttrExists
  | AttrBin
  | PseudoNthChild
  | PseudoNot
  | PseudoHas
  | PseudoIs
  | PseudoFirstChild
  | PseudoLastChild
>;

export interface SelectorParseCtx {
  root: Selector;
  current: Selector;
}

export interface MatchContext {
  /** Used for `:has()` and `:not()` */
  subSelect(selectors: MatcherFn[], idx: number): boolean;
  getFirstChild(id: number): number;
  getLastChild(id: number): number;
  getSiblings(id: number): number[];
  getParent(id: number): number;
  getField(id: number, prop: number): number;
  getType(id: number): number;
  getAttrPathValue(id: number, propIds: number[], idx: number): unknown;
}

export type MatcherFn = (ctx: MatchContext, id: number) => boolean;
export type TransformFn = (value: string) => number;

export {};
