// Verifies that the cached full packument was slimmed down before being
// written to disk: the `time` field (needed by minimumDependencyAge) is kept,
// while the per-version `scripts` map is dropped in favor of a
// `hasInstallScript` flag, matching the abbreviated install manifest shape.
const registryJson = JSON.parse(
  Deno.readTextFileSync(
    "deno_dir/npm/localhost_4260/@denotest/scripts-with-publish-date/registry.json",
  ),
);

const version = registryJson.versions["1.0.0"];

console.log("has time field:", "time" in registryJson);
console.log("has scripts:", "scripts" in version);
console.log("hasInstallScript:", version.hasInstallScript);
