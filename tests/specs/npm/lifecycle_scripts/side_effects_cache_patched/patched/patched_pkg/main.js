// This file only exists in the *patched* copy of the package. The registry
// version's `main.js` instead `require`s the `message.js` produced by its
// `postinstall` build script. Printing this marker proves the patch is applied
// and was NOT replaced by a built variant restored from the side-effects cache
// (a restored unpatched build would not contain this line). The patch is kept
// self-contained on purpose: deno does not run lifecycle scripts for linked
// (patched) packages, so the entrypoint must not depend on `postinstall` output.
console.log("patched source in use");
