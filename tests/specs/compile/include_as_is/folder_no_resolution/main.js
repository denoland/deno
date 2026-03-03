const entries = Array.from(Deno.readDirSync("./scripts")).map((e) => e.name);
entries.sort();
console.log(entries.join(","));
