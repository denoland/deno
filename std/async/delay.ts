// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
/* Resolves after the given number of milliseconds. */
export function delay(ms: number): Promise<void> {
  return new Promise((res): number =>
    setTimeout((): void => {
      res();
    }, ms)
  );
}
