// Confirms --dry-run did not modify files.
console.log("--- a/deno.json ---");
console.log(Deno.readTextFileSync("a/deno.json").trim());
console.log("--- b/deno.json ---");
console.log(Deno.readTextFileSync("b/deno.json").trim());
