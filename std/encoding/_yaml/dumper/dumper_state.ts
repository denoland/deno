// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import type { Schema, SchemaDefinition } from "../schema.ts";
import { State } from "../state.ts";
import type { StyleVariant, Type } from "../type.ts";
import type { Any, ArrayObject } from "../utils.ts";

const _hasOwnProperty = Object.prototype.hasOwnProperty;

function compileStyleMap(
  schema: Schema,
  map?: ArrayObject<StyleVariant> | null,
): ArrayObject<StyleVariant> {
  if (typeof map === "undefined" || map === null) return {};

  let type: Type;
  const result: ArrayObject<StyleVariant> = {};
  const keys = Object.keys(map);
  let tag: string, style: StyleVariant;
  for (let index = 0, length = keys.length; index < length; index += 1) {
    tag = keys[index];
    style = String(map[tag]) as StyleVariant;
    if (tag.slice(0, 2) === "!!") {
      tag = `tag:yaml.org,2002:${tag.slice(2)}`;
    }
    type = schema.compiledTypeMap.fallback[tag];

    if (
      type &&
      typeof type.styleAliases !== "undefined" &&
      _hasOwnProperty.call(type.styleAliases, style)
    ) {
      style = type.styleAliases[style];
    }

    result[tag] = style;
  }

  return result;
}

export interface DumperStateOptions {
  /** indentation width to use (in spaces). */
  indent?: number;
  /** when true, will not add an indentation level to array elements */
  noArrayIndent?: boolean;
  /**
   * do not throw on invalid types (like function in the safe schema)
   * and skip pairs and single values with such types.
   */
  skipInvalid?: boolean;
  /**
   * specifies level of nesting, when to switch from
   * block to flow style for collections. -1 means block style everywhere
   */
  flowLevel?: number;
  /** Each tag may have own set of styles.	- "tag" => "style" map. */
  styles?: ArrayObject<StyleVariant> | null;
  /** specifies a schema to use. */
  schema?: SchemaDefinition;
  /**
   * If true, sort keys when dumping YAML in ascending, ASCII character order.
   * If a function, use the function to sort the keys. (default: false)
   * If a function is specified, the function must return a negative value
   * if first argument is less than second argument, zero if they're equal
   * and a positive value otherwise.
   */
  sortKeys?: boolean | ((a: string, b: string) => number);
  /** set max line width. (default: 80) */
  lineWidth?: number;
  /**
   * if true, don't convert duplicate objects
   * into references (default: false)
   */
  noRefs?: boolean;
  /**
   * if true don't try to be compatible with older yaml versions.
   * Currently: don't quote "yes", "no" and so on,
   * as required for YAML 1.1 (default: false)
   */
  noCompatMode?: boolean;
  /**
   * if true flow sequences will be condensed, omitting the
   * space between `key: value` or `a, b`. Eg. `'[a,b]'` or `{a:{b:c}}`.
   * Can be useful when using yaml for pretty URL query params
   * as spaces are %-encoded. (default: false).
   */
  condenseFlow?: boolean;
}

export class DumperState extends State {
  public indent: number;
  public noArrayIndent: boolean;
  public skipInvalid: boolean;
  public flowLevel: number;
  public sortKeys: boolean | ((a: Any, b: Any) => number);
  public lineWidth: number;
  public noRefs: boolean;
  public noCompatMode: boolean;
  public condenseFlow: boolean;
  public implicitTypes: Type[];
  public explicitTypes: Type[];
  public tag: string | null = null;
  public result = "";
  public duplicates: Any[] = [];
  public usedDuplicates: Any[] = []; // changed from null to []
  public styleMap: ArrayObject<StyleVariant>;
  public dump: Any;

  constructor({
    schema,
    indent = 2,
    noArrayIndent = false,
    skipInvalid = false,
    flowLevel = -1,
    styles = null,
    sortKeys = false,
    lineWidth = 80,
    noRefs = false,
    noCompatMode = false,
    condenseFlow = false,
  }: DumperStateOptions) {
    super(schema);
    this.indent = Math.max(1, indent);
    this.noArrayIndent = noArrayIndent;
    this.skipInvalid = skipInvalid;
    this.flowLevel = flowLevel;
    this.styleMap = compileStyleMap(this.schema as Schema, styles);
    this.sortKeys = sortKeys;
    this.lineWidth = lineWidth;
    this.noRefs = noRefs;
    this.noCompatMode = noCompatMode;
    this.condenseFlow = condenseFlow;

    this.implicitTypes = (this.schema as Schema).compiledImplicit;
    this.explicitTypes = (this.schema as Schema).compiledExplicit;
  }
}
