// Relaxed default profile (gate on, no flags): read is confined to the cwd and
// the OS temp directory. Loading a module that lives in a managed npm package
// grants read on that package's own folder, scoped to packages this program
// actually imports.
//
// DENO_DIR (where the packages are cached) is a sibling of the cwd under the OS
// temp root, and TMPDIR is redirected to a subdir of the cwd, so DENO_DIR is
// outside both the cwd read grant and the temp read grant. A package folder is
// therefore only readable when the per-package grant fires.
import pkgA from "@denotest/relaxed-pkg-a";

// Package A was imported, so its folder was granted: it can read its own
// bundled data file at runtime without a prompt.
console.log(`A read own data: ${pkgA.readOwnData().trim()}`);

// Package B was cached by setup but is never imported here, so its folder is
// not granted. Its absolute path was recorded into a cwd file (readable because
// it is inside the cwd), but reading the file itself is denied.
const bDataPath = Deno.readTextFileSync("./b_data_path.txt").trim();
try {
  Deno.readTextFileSync(bDataPath);
  console.log("B read: UNEXPECTEDLY ALLOWED");
} catch (err) {
  const named = err.message.includes("--allow-read") ? "--allow-read" : "?";
  console.log(`B read: ${err.name} ${named}`);
}
