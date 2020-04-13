// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
interface Version {
  deno: string;
  v8: string;
  typescript: string;
}

export const version: Version = {
  deno: "",
  v8: "",
  typescript: "",
};

export function setVersions(
  denoVersion: string,
  v8Version: string,
  tsVersion: string
): void {
  version.deno = denoVersion;
  version.v8 = v8Version;
  version.typescript = tsVersion;

  Object.freeze(version);
}
