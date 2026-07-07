// Assert that the OOM run in the previous step left a `.heapsnapshot` file in
// the (shared) temp working directory.
let found: string | undefined;
for (const entry of Deno.readDirSync(".")) {
  if (entry.isFile && entry.name.endsWith(".heapsnapshot")) {
    found = entry.name;
    break;
  }
}
if (found === undefined) {
  console.error("no .heapsnapshot file found");
  Deno.exit(1);
}
// Make sure it is a parseable JSON heap snapshot with the expected shape.
const parsed = JSON.parse(Deno.readTextFileSync(found));
if (parsed.snapshot === undefined || !Array.isArray(parsed.nodes)) {
  console.error("heap snapshot is not a valid snapshot document");
  Deno.exit(1);
}
console.log("ok");
