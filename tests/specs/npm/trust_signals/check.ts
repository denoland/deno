// The full packument carries a large `_npmUser.trustedPublisher` object, but
// only its presence is a trust signal. Verify that the cached registry.json
// kept just a compact marker (dropping the heavy sub-fields) and that the
// derived publishing-trust rank was recorded in the lockfile.
const registryJson = JSON.parse(
  Deno.readTextFileSync(
    "deno_dir/npm/localhost_4260/@denotest/trusted-publisher/registry.json",
  ),
);
const version = registryJson.versions["1.0.0"];
console.log("trustedPublisher marker:", version._npmUser?.trustedPublisher);
console.log(
  "heavy sub-fields dropped:",
  !JSON.stringify(version).includes("oidcConfigId"),
);

const lockfile = JSON.parse(Deno.readTextFileSync("deno.lock"));
const entry = Object.entries(lockfile.npm ?? {}).find(([key]) =>
  key.startsWith("@denotest/trusted-publisher@")
);
console.log("lockfile trust rank:", (entry?.[1] as { trust?: number })?.trust);
