// Importing the package runs its `main.js`, which `require`s the `message.js`
// file produced by the `postinstall` build script. This drives a normal install
// (with `--allow-scripts`) so `node_modules/.bin` is set up and the package's
// bin entrypoint is chmodded — the step that previously corrupted the pristine
// global cache.
import "npm:@denotest/lifecycle-scripts-simple@1.0.0/main.js";
