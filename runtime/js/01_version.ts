// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { primordials } = globalThis.__bootstrap;
const {
  ObjectFreeze,
} = primordials;

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

return { setVersions, version };
})();
