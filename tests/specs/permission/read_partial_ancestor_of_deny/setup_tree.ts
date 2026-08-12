// Build a tree where the denied path lives *inside* the dir we later try to
// remove recursively. The tree is created with full --allow-write so the deny
// scope only applies during the recursive-remove step.
Deno.mkdirSync("parent/denied", { recursive: true });
Deno.writeTextFileSync("parent/denied/keep.txt", "keep");
Deno.writeTextFileSync("parent/other.txt", "other");
console.log("setup ok");
