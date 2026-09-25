// Importing the package runs its `main.js`, which `require`s the `message.js`
// file produced by the `postinstall` build script. If the build output is
// present (either freshly built or restored from the side-effects cache) this
// prints "postinstall works".
import "npm:@denotest/lifecycle-scripts-simple@1.0.0/main.js";
