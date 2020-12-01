// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export type mkdtempCallback = (
  err: Error | undefined,
  directory?: string,
) => void;

// TODO - 'encoding' handling needs implementation in Deno.makeTempDir
export function mkdtemp(
  prefix: string,
  optionsOrCallback: { encoding: string } | string | mkdtempCallback,
  callback?: mkdtempCallback,
): void {
  const callbackFn: mkdtempCallback | undefined =
    optionsOrCallback instanceof Function ? optionsOrCallback : callback;
  //const encoding: string | undefined =
  //    optionsOrCallback instanceof Function ? undefined : optionsOrCallback?.encoding || optionsOrCallback;

  if (!callbackFn) {
    throw new Error("No callback function supplied");
  }

  Deno.makeTempDir({ dir: prefix })
    .then((directory) => callbackFn(undefined, directory))
    .catch((error) => callbackFn(error));
}
