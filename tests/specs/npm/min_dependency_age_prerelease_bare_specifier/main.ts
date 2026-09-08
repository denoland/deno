// Same as `min_dependency_age_prerelease_dist_tag`, but with a bare specifier
// (no version, so `*`). The `latest` tag points at a prerelease (1.0.0-dev.2)
// published too recently for the configured minimum dependency age, and a plain
// `*` never matches a prerelease, so this used to fail outright with "Could not
// find npm package ... matching '*'". It should fall back to the newest allowed
// version at or below the tagged version (1.0.0-dev.1), just like `@latest`.
// Regression test for https://github.com/denoland/deno/issues/36614.
import pkg from "npm:@denotest/pre-release-latest-min-age";
console.log("resolved:", pkg.version);
