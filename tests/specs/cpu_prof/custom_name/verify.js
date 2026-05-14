const data = JSON.parse(Deno.readTextFileSync("custom.cpuprofile"));
if (!Array.isArray(data.nodes) || data.nodes.length === 0) {
  console.log("Invalid profile");
  Deno.exit(1);
}
console.log("Valid profile");
