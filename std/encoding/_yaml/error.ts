// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import type { Mark } from "./mark.ts";

export class YAMLError extends Error {
  constructor(
    message = "(unknown reason)",
    protected mark: Mark | string = "",
  ) {
    super(`${message} ${mark}`);
    this.name = this.constructor.name;
  }

  public toString(_compact: boolean): string {
    return `${this.name}: ${this.message} ${this.mark}`;
  }
}
