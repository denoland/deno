// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2026 the Deno authors. MIT license.
// This module is browser compatible.

/**
 * The kind of YAML node.
 */
export type KindType = "sequence" | "scalar" | "mapping";
/**
 * The style variation for `styles` option of {@linkcode stringify}
 */
export type StyleVariant =
  | "lowercase"
  | "uppercase"
  | "camelcase"
  | "decimal"
  | "binary"
  | "octal"
  | "hexadecimal";

/**
 * Function to convert data to a string for YAML serialization.
 */
export type RepresentFn<D> = (data: D, style?: StyleVariant) => string;

/**
 * A type definition for a YAML node.
 */
// deno-lint-ignore no-explicit-any
export interface Type<K extends KindType, D = any> {
  /** Tag to identify the type */
  tag: string;
  /** Kind of type */
  kind: K;
  /** Cast the type. Used to stringify */
  predicate?: (data: unknown) => data is D;
  /** Function to represent data. Used to stringify */
  represent?: RepresentFn<D> | Record<string, RepresentFn<D>>;
  /** Default style for the type. Used to stringify */
  defaultStyle?: StyleVariant;
  /** Function to test whether data can be resolved by this type. Used to parse */
  // deno-lint-ignore no-explicit-any
  resolve: (data: any) => boolean;
  /** Function to construct data from string. Used to parse */
  // deno-lint-ignore no-explicit-any
  construct: (data: any) => D;
}
