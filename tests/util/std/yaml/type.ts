// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import type { Any, ArrayObject } from "./_utils.ts";

export type KindType = "sequence" | "scalar" | "mapping";
export type StyleVariant = "lowercase" | "uppercase" | "camelcase" | "decimal";
export type RepresentFn = (data: Any, style?: StyleVariant) => Any;

interface TypeOptions {
  kind: KindType;
  resolve?: (data: Any) => boolean;
  construct?: (data: string) => Any;
  instanceOf?: Any;
  predicate?: (data: Record<string, unknown>) => boolean;
  represent?: RepresentFn | ArrayObject<RepresentFn>;
  defaultStyle?: StyleVariant;
  styleAliases?: ArrayObject;
}

function checkTagFormat(tag: string): string {
  return tag;
}

export class Type {
  public tag: string;
  public kind: KindType | null = null;
  public instanceOf: Any;
  public predicate?: (data: Record<string, unknown>) => boolean;
  public represent?: RepresentFn | ArrayObject<RepresentFn>;
  public defaultStyle?: StyleVariant;
  public styleAliases?: ArrayObject;
  public loadKind?: KindType;

  constructor(tag: string, options?: TypeOptions) {
    this.tag = checkTagFormat(tag);
    if (options) {
      this.kind = options.kind;
      this.resolve = options.resolve || (() => true);
      this.construct = options.construct || ((data: Any): Any => data);
      this.instanceOf = options.instanceOf;
      this.predicate = options.predicate;
      this.represent = options.represent;
      this.defaultStyle = options.defaultStyle;
      this.styleAliases = options.styleAliases;
    }
  }
  public resolve: (data?: Any) => boolean = (): boolean => true;
  public construct: (data?: Any) => Any = (data): Any => data;
}
