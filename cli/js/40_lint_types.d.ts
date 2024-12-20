// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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

export interface LintState {
  plugins: Deno.LintPlugin[];
  installedPlugins: Set<string>;
}

export type VisitorFn = (node: Deno.AstNode) => void;

export interface CompiledVisitor {
  matcher: (offset: number) => boolean;
  info: { enter: VisitorFn; exit: VisitorFn };
}

export {};
