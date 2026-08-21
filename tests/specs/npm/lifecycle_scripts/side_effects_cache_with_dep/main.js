// Importing the package runs its `main.js`, which both `require`s the
// `generated.js` file produced by its `postinstall` build script AND `require`s
// one of its runtime dependencies (`@denotest/add`). This exercises the
// side-effects cache restore path for a package that HAS dependencies: a cache
// hit must leave both the generated build output AND the dependency symlink in
// place, otherwise the dependency `require` throws.
import "npm:@denotest/lifecycle-scripts-with-dep@1.0.0/main.js";
