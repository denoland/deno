// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as domTypes from "./dom_types.d.ts";

export function getDOMStringList(arr: string[]): domTypes.DOMStringList {
  Object.defineProperties(arr, {
    contains: {
      value(searchElement: string): boolean {
        return arr.includes(searchElement);
      },
      enumerable: true,
    },
    item: {
      value(idx: number): string | null {
        return idx in arr ? arr[idx] : null;
      },
    },
  });
  return arr as string[] & domTypes.DOMStringList;
}
