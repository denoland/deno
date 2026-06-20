// Simulate a stale npm registry metadata cache. The user installed
// @denotest/has-patch-versions back when only 0.1.0 existed, so their cached
// packument predates the newer in-range 0.1.1 (and the out-of-range 0.2.0) and
// their lockfile is pinned at 0.1.0. `deno update --lockfile-only` must still
// pick up 0.1.1 by refetching metadata for the package. See denoland/deno#35348.

// Drop 0.1.1 and 0.2.0 from the cached packument so the npm registry cache no
// longer knows about any version newer than 0.1.0. The host directory is the
// fixed test registry port (PUBLIC_NPM_REGISTRY_PORT = 4260).
const registryPath =
  "deno_dir/npm/localhost_4260/@denotest/has-patch-versions/registry.json";
const registry = JSON.parse(Deno.readTextFileSync(registryPath));
delete registry.versions["0.1.1"];
delete registry.versions["0.2.0"];
if (registry["dist-tags"]) {
  registry["dist-tags"].latest = "0.1.0";
}
// Drop the cached etag. In the real-world scenario the cached packument
// predates 0.1.1, so its stored etag no longer matches the registry and the
// conditional refetch returns the updated packument. Here the test registry
// always serves the full packument, so a kept etag would yield a spurious 304
// Not Modified and hand back this truncated cache. Removing it makes the
// refetch unconditional, mirroring a genuinely stale cache entry.
delete registry["_deno.etag"];
Deno.writeTextFileSync(registryPath, JSON.stringify(registry));

// Pin the lockfile back at the outdated 0.1.0.
const lockPath = "deno.lock";
const lock = JSON.parse(Deno.readTextFileSync(lockPath));
lock.specifiers["npm:@denotest/has-patch-versions@0.1"] = "0.1.0";
lock.npm = {
  "@denotest/has-patch-versions@0.1.0": {
    "integrity":
      "sha512-H/MBo0jKDdMsX4AAGEGQbZj70nfNe3oUNZXbohYHhqf9EfpLnXp/7FC29ZdfV4+p6VjEcOGdCtXc6rilE6iYpg==",
  },
};
Deno.writeTextFileSync(lockPath, JSON.stringify(lock, null, 2) + "\n");
