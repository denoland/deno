// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
interface Version {
  deno: string | null;
  v8: string | null;
  typescript: string;
}

export const version: Version = {
  deno: null,
  v8: null,
  typescript: "TS_VERSION"
};

/**
 * Sets the deno and v8 versions and freezes the version object.
 */
export function setVersions(denoVersion: string, v8Version: string): void {
  version.deno = denoVersion;
  version.v8 = v8Version;

  Object.freeze(version);
}
