// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
interface Version {
  deno: string;
  v8: string;
  typescript: string;
}

export const version: Version = {
  deno: "",
  v8: "",
  // This string will be replaced by rollup
  typescript: `ROLLUP_REPLACE_TS_VERSION`
};

/**
 * Sets the deno and v8 versions and freezes the version object.
 * @internal
 */
export function setVersions(denoVersion: string, v8Version: string): void {
  version.deno = denoVersion;
  version.v8 = v8Version;

  Object.freeze(version);
}
