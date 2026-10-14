// Verifies which packument format ended up in the cache.
for (const name of ["min-release-age-old", "min-release-age-latest"]) {
  const registryJson = JSON.parse(
    Deno.readTextFileSync(
      `deno_dir/npm/localhost_4260/@denotest/${name}/registry.json`,
    ),
  );
  console.log(name);
  console.log("  publish dates:", Object.keys(registryJson.time).length);
  console.log("  modified:", registryJson.modified);
  console.log("  packument format:", registryJson["_deno.packumentFormat"]);
}
