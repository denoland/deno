// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ObjectFreeze,
} = primordials;

interface Version {
  deno: string;
  v8: string;
  typescript: string;
}

const version: Version = {
  deno: "",
  v8: "",
  typescript: "",
};

function setVersions(
  denoVersion,
  v8Version,
  tsVersion,
) {
  version.deno = denoVersion;
  version.v8 = v8Version;
  version.typescript = tsVersion;

  ObjectFreeze(version);
}

export { setVersions, version };
