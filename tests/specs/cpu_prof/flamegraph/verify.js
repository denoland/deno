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
if (!content.includes('id="frames"')) {
  console.log("Missing frames container");
  Deno.exit(1);
}
if (!content.includes("fg:x=")) {
  console.log("Missing frame data attributes");
  Deno.exit(1);
}
if (!content.includes("function init(evt)")) {
  console.log("Missing interactive JavaScript");
  Deno.exit(1);
}
if (!content.includes('height="100%"')) {
  console.log("SVG should use full viewport height");
  Deno.exit(1);
}
if (!content.includes("fg:content_height=")) {
  console.log("Missing content height data attribute");
  Deno.exit(1);
}
console.log("Valid flamegraph");
