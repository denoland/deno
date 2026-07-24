// A normal (unpatched) install of the package. Its `postinstall` build script
// runs and the output is snapshotted into the shared global side-effects cache,
// creating a pristine cache dir + built variant for this exact name@version.
import "npm:@denotest/lifecycle-scripts-simple@1.0.0/main.js";
