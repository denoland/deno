try {
  await Deno.open("./some/file.txt");
} catch (_e) {}

// deno-lint-ignore no-explicit-any
function _foo(): any {}
