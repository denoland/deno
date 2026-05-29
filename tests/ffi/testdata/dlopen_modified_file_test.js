// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/20956.
//
// Before #20956 was fixed, dlopen()ing a file and then overwriting that file
// (e.g. via `Deno.copyFileSync` which truncates the destination in place)
// corrupted the in-memory mapping of the loaded library and crashed the
// process at exit when ld.so ran the library's finalizers.

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const [libPrefix, libSuffix] = {
  darwin: ["lib", "dylib"],
  linux: ["lib", "so"],
  windows: ["", "dll"],
}[Deno.build.os];
const libPath = `${targetDir}/${libPrefix}test_ffi.${libSuffix}`;

const f = Deno.makeTempFileSync({ suffix: `.${libSuffix}` });
Deno.copyFileSync(libPath, f);

// Mirror the reproduction from the issue: dlopen with no symbols, then
// overwrite the same path. In the buggy build, the second `copyFileSync`
// truncates and rewrites the inode currently mapped into our address space.
Deno.dlopen(f, {});
Deno.copyFileSync(libPath, f);

console.log("dlopen_modified_file_test passed");

// Clean up the temp file the user controls. Our copy used for dlopen lives
// on a different (unlinked) inode, so this removal does not affect it.
try {
  Deno.removeSync(f);
} catch { /* ignore */ }
