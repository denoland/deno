// Simulate a stale npm registry metadata cache whose newest known version is
// *older* than the version already pinned in the lockfile. The user installed
// @denotest/has-patch-versions@0.2.0 (the newest, via the `*` range), but their
// cached packument later went stale and no longer lists 0.2.0. `deno update
// --lockfile-only` must NOT downgrade the lockfile: it has to refetch metadata
// for the package and keep 0.2.0. Before the fix the installer re-resolved
// against the stale cache and silently downgraded 0.2.0 -> 0.1.1. See
// denoland/deno#35822.

// Drop 0.2.0 from the cached packument so the npm registry cache no longer
// knows about the version the lockfile is pinned at. The host directory is the
// fixed test registry port (PUBLIC_NPM_REGISTRY_PORT = 4260).
const registryPath =
  "deno_dir/npm/localhost_4260/@denotest/has-patch-versions/registry.json";
const registry = JSON.parse(Deno.readTextFileSync(registryPath));
delete registry.versions["0.2.0"];
if (registry["dist-tags"]) {
  registry["dist-tags"].latest = "0.1.1";
}
// Drop the cached etag so the conditional refetch is unconditional, mirroring a
// genuinely stale cache entry whose stored etag no longer matches the registry.
// (The test registry always serves the full packument, so a kept etag would
// yield a spurious 304 Not Modified and hand back this truncated cache.)
delete registry["_deno.etag"];
Deno.writeTextFileSync(registryPath, JSON.stringify(registry));
