// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export function getDOMStringList(arr: string[]): DOMStringList {
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
  return arr as string[] & DOMStringList;
}
