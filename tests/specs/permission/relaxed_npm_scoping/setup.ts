// Runs with -A so the relaxed profile is off. Caches both npm packages into
// DENO_DIR and records package B's bundled data-file path so the confined run
// can attempt to read it without importing B.
import pkgA from "@denotest/relaxed-pkg-a";
import pkgB from "@denotest/relaxed-pkg-b";

// Touch A so it is cached too (the confined run imports it).
void pkgA.dataPath;

Deno.writeTextFileSync("./b_data_path.txt", pkgB.dataPath);
console.log("setup: ok");
