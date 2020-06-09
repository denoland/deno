try {
  await Deno.open("./some/file.txt");
} catch (e) {}

// deno-lint-ignore no-explicit-any require-await
function foo(): any {}

// deno-lint-ignore
while (false) {}
