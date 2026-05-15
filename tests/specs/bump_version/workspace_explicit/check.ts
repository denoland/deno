// Verifies post-bump file contents.
console.log("--- a/deno.json ---");
console.log(Deno.readTextFileSync("a/deno.json").trim());
console.log("--- b/deno.json ---");
console.log(Deno.readTextFileSync("b/deno.json").trim());
console.log("--- import_map.json ---");
console.log(Deno.readTextFileSync("import_map.json").trim());
