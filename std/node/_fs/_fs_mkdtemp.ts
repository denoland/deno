// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export type mkdtempCallback = (
  err: Error | undefined,
  directory?: string,
) => void;

export function mkdtemp(prefix: string, callback: mkdtempCallback): void;
export function mkdtemp(
  prefix: string,
  options: { encoding: string } | string,
  callback: mkdtempCallback,
): void;
// TODO - 'encoding' handling needs implementation in Deno.makeTempDir
export function mkdtemp(
  prefix: string,
  optionsOrCallback: { encoding: string } | string | mkdtempCallback,
  maybeCallback?: mkdtempCallback,
): void {
  const callback: mkdtempCallback | undefined =
    optionsOrCallback instanceof Function ? optionsOrCallback : maybeCallback;

  if (!callback) throw new Error("No callback function supplied");

  Deno.makeTempDir({ dir: prefix })
    .then((directory) => callback(undefined, directory))
    .catch((error) => callback(error));
}

// TODO - 'encoding' handling needs implementation in Deno.makeTempDirSync
export function mkdtempSync(
  prefix: string,
  options?: { encoding: string } | string,
): string {
  return Deno.makeTempDirSync({ dir: prefix });
}
