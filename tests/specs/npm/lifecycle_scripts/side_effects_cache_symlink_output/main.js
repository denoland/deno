// The package's `postinstall` creates a `linked.js` SYMLINK (pointing at the
// shipped `real.js`) inside its own directory, and its `main.js` requires
// through that symlink. The side-effects cache snapshots/restores with
// `clone_dir_recursive`, which can't represent symlinks — so the package must
// be left uncacheable, and its build script must re-run on the second install
// to recreate the link. If the cache wrongly served a variant, `linked.js`
// would be missing and this import would throw ERR_MODULE_NOT_FOUND.
import value from "npm:@denotest/lifecycle-scripts-symlink@1.0.0/main.js";
console.log(value);
