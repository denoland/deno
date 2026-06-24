// When the offending package is a declared package.json dependency, the
// top-level install resolves (and warns about) it before the module graph is
// built, so the warning is emitted without importer attribution even though
// this module imports it. The package.json is the source in that case. See the
// discussion on https://github.com/denoland/deno/pull/35242.
import "npm:@denotest/peer-dep-specific-constraint";
