// The full packument carries large `_npmUser.trustedPublisher` and
// `dist.attestations.provenance` objects, but only their presence is a trust
// signal. Verify the cached registry.json kept just compact markers (dropping
// the heavy sub-fields) so caching the full packument by default stays cheap.
const registryJson = JSON.parse(
  Deno.readTextFileSync(
    "deno_dir/npm/localhost_4260/@denotest/trusted-publisher/registry.json",
  ),
);
const version = registryJson.versions["1.0.0"];
console.log("trustedPublisher marker:", version._npmUser?.trustedPublisher);
console.log("provenance marker:", version.dist?.attestations?.provenance);
console.log(
  "heavy sub-fields dropped:",
  !JSON.stringify(version).includes("oidcConfigId") &&
    !JSON.stringify(version).includes("predicateType"),
);
