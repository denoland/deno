// `latest` points at a prerelease (1.0.0-dev.2) that was published too recently
// for the configured minimum dependency age. Resolution should fall back to the
// newest allowed version at or below the tagged version (1.0.0-dev.1) instead of
// failing the tag resolution outright.
// Regression test for https://github.com/denoland/deno/issues/35552.
import pkg from "npm:@denotest/pre-release-latest-min-age@latest";
console.log("resolved:", pkg.version);
