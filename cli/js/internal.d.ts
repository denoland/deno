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

export enum AttrOp {
  /** [attr="value"] or [attr=value] */
  Equal,
  /** [attr!="value"] or [attr!=value] */
  NotEqual,
  /** [attr>1] */
  Greater,
  /** [attr>=1] */
  GreaterThan,
  /** [attr<1] */
  Less,
  /** [attr<=1] */
  LessThan,
}

export interface AttrExists {
  type: 3;
  prop: number;
  debug?: string;
}

export interface AttrBin {
  type: 4;
  prop: number;
  op: AttrOp;
  value: string;
}

export interface AttrRegex {
  type: 5;
  prop: number;
  value: RegExp;
}

export type AttrSelector = AttrExists | AttrBin | AttrRegex;

export interface Elem {
  type: 1;
  wildcard: boolean;
  elem: number;
  debug?: string;
}

export interface PseudoNthChild {
  type: 6;
  backward: boolean;
  step: number;
  stepOffset: number;
  of: Selector | null;
}

export interface PseudoHas {
  type: 7;
  selector: Selector[];
}
export interface PseudoNot {
  type: 8;
  selector: Selector[];
}
export interface PseudoFirstChild {
  type: 9;
}
export interface PseudoLastChild {
  type: 10;
}

export interface Relation {
  type: 2;
  op: number;
  debug?: string;
}

export type Selector = Array<
  | Elem
  | Relation
  | AttrExists
  | AttrBin
  | AttrRegex
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

export enum SelToken {
  Value = 0,
  Char = 1,
  EOF = 2,
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
  hasAttr(id: number, propId: number): boolean;
  getAttrValue(id: number, propId: number): unknown;
}

export type NextFn = (ctx: MatchCtx, id: number) => boolean;
export type MatcherFn = (ctx: MatchCtx, id: number) => boolean;
