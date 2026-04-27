const files = [...Deno.readDirSync(".")].filter((f) =>
  f.name.endsWith(".cpuprofile")
);
if (files.length === 0) {
  console.log("No .cpuprofile files found");
  Deno.exit(1);
}
const data = JSON.parse(Deno.readTextFileSync(files[0].name));
if (!Array.isArray(data.nodes) || data.nodes.length === 0) {
  console.log("Invalid profile: missing nodes");
  Deno.exit(1);
}
if (typeof data.startTime !== "number" || typeof data.endTime !== "number") {
  console.log("Invalid profile: missing timestamps");
  Deno.exit(1);
}
console.log("Valid profile with", data.nodes.length, "nodes");
