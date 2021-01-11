// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { repeat } from "./utils.ts";

export class Mark {
  constructor(
    public name: string,
    public buffer: string,
    public position: number,
    public line: number,
    public column: number,
  ) {}

  public getSnippet(indent = 4, maxLength = 75): string | null {
    if (!this.buffer) return null;

    let head = "";
    let start = this.position;

    while (
      start > 0 &&
      "\x00\r\n\x85\u2028\u2029".indexOf(this.buffer.charAt(start - 1)) === -1
    ) {
      start -= 1;
      if (this.position - start > maxLength / 2 - 1) {
        head = " ... ";
        start += 5;
        break;
      }
    }

    let tail = "";
    let end = this.position;

    while (
      end < this.buffer.length &&
      "\x00\r\n\x85\u2028\u2029".indexOf(this.buffer.charAt(end)) === -1
    ) {
      end += 1;
      if (end - this.position > maxLength / 2 - 1) {
        tail = " ... ";
        end -= 5;
        break;
      }
    }

    const snippet = this.buffer.slice(start, end);
    return `${repeat(" ", indent)}${head}${snippet}${tail}\n${
      repeat(
        " ",
        indent + this.position - start + head.length,
      )
    }^`;
  }

  public toString(compact?: boolean): string {
    let snippet,
      where = "";

    if (this.name) {
      where += `in "${this.name}" `;
    }

    where += `at line ${this.line + 1}, column ${this.column + 1}`;

    if (!compact) {
      snippet = this.getSnippet();

      if (snippet) {
        where += `:\n${snippet}`;
      }
    }

    return where;
  }
}
