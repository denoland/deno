/**
 * @license
 * Copyright (c) 2018-2020 The Deno Authors. All rights reserved.
 * This code may only be used under the MIT license.
 */

/* Resolves after the given number of milliseconds. */
export function delay(ms: number): Promise<void> {
  return new Promise((res): number =>
    setTimeout((): void => {
      res();
    }, ms)
  );
}
