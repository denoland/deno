// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import type { YAMLError } from "../error.ts";
import type { Schema, SchemaDefinition, TypeMap } from "../schema.ts";
import { State } from "../state.ts";
import type { Type } from "../type.ts";
import type { Any, ArrayObject } from "../utils.ts";

export interface LoaderStateOptions {
  legacy?: boolean;
  listener?: ((...args: Any[]) => void) | null;
  /** string to be used as a file path in error/warning messages. */
  filename?: string;
  /** specifies a schema to use. */
  schema?: SchemaDefinition;
  /** compatibility with JSON.parse behaviour. */
  json?: boolean;
  /** function to call on warning messages. */
  onWarning?(this: null, e?: YAMLError): void;
}

// deno-lint-ignore no-explicit-any
export type ResultType = any[] | Record<string, any> | string;

export class LoaderState extends State {
  public documents: Any[] = [];
  public length: number;
  public lineIndent = 0;
  public lineStart = 0;
  public position = 0;
  public line = 0;
  public filename?: string;
  public onWarning?: (...args: Any[]) => void;
  public legacy: boolean;
  public json: boolean;
  public listener?: ((...args: Any[]) => void) | null;
  public implicitTypes: Type[];
  public typeMap: TypeMap;

  public version?: string | null;
  public checkLineBreaks?: boolean;
  public tagMap?: ArrayObject;
  public anchorMap?: ArrayObject;
  public tag?: string | null;
  public anchor?: string | null;
  public kind?: string | null;
  public result: ResultType | null = "";

  constructor(
    public input: string,
    {
      filename,
      schema,
      onWarning,
      legacy = false,
      json = false,
      listener = null,
    }: LoaderStateOptions,
  ) {
    super(schema);
    this.filename = filename;
    this.onWarning = onWarning;
    this.legacy = legacy;
    this.json = json;
    this.listener = listener;

    this.implicitTypes = (this.schema as Schema).compiledImplicit;
    this.typeMap = (this.schema as Schema).compiledTypeMap;

    this.length = input.length;
  }
}
