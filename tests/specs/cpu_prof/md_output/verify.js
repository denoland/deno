const files = [...Deno.readDirSync(".")].filter((f) => f.name.endsWith(".md"));
if (files.length === 0) {
  console.log("No .md files found");
  Deno.exit(1);
}
const content = Deno.readTextFileSync(files[0].name);
if (!content.includes("# CPU Profile")) {
  console.log("Missing header");
  Deno.exit(1);
}
if (!content.includes("## Hot Functions")) {
  console.log("Missing Hot Functions section");
  Deno.exit(1);
}
console.log("Valid markdown report");
