// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const primordials = globalThis.__bootstrap.primordials;
const { ObjectFreeze } = primordials;

const version = {
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
