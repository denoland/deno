const importsDir = Deno.readDirSync(Deno.realPathSync("./tla2"));

for (const { name } of importsDir) {
  const filePath = Deno.realPathSync(`./tla2/${name}`);
  console.log("loading", filePath);
  const mod = await import(`file://${filePath}`);
  console.log("loaded", mod);
}

console.log("all loaded");
