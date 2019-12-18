// Ported from js-yaml v3.13.1:
// https://github.com/nodeca/js-yaml/commit/665aadda42349dcae869f12040d9b10ef18d12da
// Copyright 2011-2015 by Vitaly Puzrin. All rights reserved. MIT license.
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/* eslint-disable-next-line @typescript-eslint/no-explicit-any */
export type Any = any;

export function repeat(str: string, count: number): string {
  return str.repeat(count);
}

export interface ArrayObject<T = Any> {
  [P: string]: T;
}
