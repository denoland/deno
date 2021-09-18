const importsDir = Deno.readDirSync(Deno.realPathSync("./tla2"));

const resolvedPaths = [];

for (const { name } of importsDir) {
  const filePath = Deno.realPathSync(`./tla2/${name}`);
  resolvedPaths.push(filePath);
}

resolvedPaths.sort();

for (const filePath of resolvedPaths) {
  console.log("loading", filePath);
  const mod = await import(`file://${filePath}`);
  console.log("loaded", mod);
}

console.log("all loaded");
