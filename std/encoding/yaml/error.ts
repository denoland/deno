// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { Mark } from "./mark.ts";

const { DenoError, ErrorKind } = Deno;

export class YAMLError extends DenoError<typeof ErrorKind.Other> {
  constructor(
    message = "(unknown reason)",
    protected mark: Mark | string = ""
  ) {
    super(ErrorKind.Other, `${message} ${mark}`);
    this.name = this.constructor.name;
  }

  public toString(_compact: boolean): string {
    return `${this.name}: ${this.message} ${this.mark}`;
  }
}
