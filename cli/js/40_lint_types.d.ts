// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

export interface AstContext {
  buf: Uint8Array;
  strTable: Map<number, string>;
  strTableOffset: number;
  rootId: number;
  nodes: Map<number, NodeFacade>;
  strByType: number[];
  strByProp: number[];
  typeByStr: Map<string, number>;
  propByStr: Map<string, number>;
}

export type VisitorFn = (node: Deno.AstNode) => void;

export interface CompiledVisitor {
  matcher: (offset: number) => boolean;
  info: { enter: VisitorFn; exit: VisitorFn };
}

export {};
