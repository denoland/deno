const entries = Array.from(
  Deno.readDirSync(new URL("./node_modules", import.meta.url)),
);
const names = entries.map((entry) => entry.name);
names.sort();

// won't have the invalid path alias
for (const name of names) {
  console.log(name);
}
