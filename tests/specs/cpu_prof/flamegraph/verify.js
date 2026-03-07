const files = [...Deno.readDirSync(".")].filter((f) => f.name.endsWith(".svg"));
if (files.length === 0) {
  console.log("No .svg files found");
  Deno.exit(1);
}
const content = Deno.readTextFileSync(files[0].name);
if (!content.includes("<svg")) {
  console.log("Missing SVG element");
  Deno.exit(1);
}
if (!content.includes("CPU Flamegraph")) {
  console.log("Missing flamegraph title");
  Deno.exit(1);
}
if (!content.includes('class="frame"')) {
  console.log("Missing flame frames");
  Deno.exit(1);
}
console.log("Valid flamegraph");
