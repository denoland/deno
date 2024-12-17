const mathDir = import.meta.dirname + "/math";
const files = Array.from(
  Deno.readDirSync(mathDir).map((entry) => mathDir + "/" + entry.name),
);
files.sort();
for (const file of files) {
  console.log(file);
}

function nonAnalyzable() {
  return "./src/main.ts";
}

await import(nonAnalyzable());
