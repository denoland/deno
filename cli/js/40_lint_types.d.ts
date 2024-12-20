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
}

// TODO(@marvinhagemeister) Remove once we land "official" types
export interface LintContext {
  report(node: unknown): void;
}

// TODO(@marvinhagemeister) Remove once we land "official" types
export interface LintRule {
  create(ctx: LintContext): Record<string, (node: unknown) => void>;
  destroy?(ctx: LintContext): void;
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
  matcher: (offset: number) => boolean;
  info: { enter: VisitorFn; exit: VisitorFn };
}

export {};
