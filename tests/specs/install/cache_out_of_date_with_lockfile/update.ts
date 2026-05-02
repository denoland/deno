const path = Deno.args[0] + "/npm/localhost_4260/@denotest/bin/registry.json";
const fileText = Deno.readTextFileSync(path);
const data = JSON.parse(fileText);
if (data.versions["1.0.0"] == null || data["dist-tags"].latest !== "1.0.0") {
  throw new Error("Test relies on version 1.0.0 to be the latest version");
}
delete data.versions["1.0.0"];
data["dist-tags"].latest = "0.5.0";
delete data["_deno.etag"];
Deno.writeTextFileSync(path, JSON.stringify(data));
Deno.remove("node_modules", { recursive: true });

// assert this exists
if (!Deno.statSync("deno.lock").isFile) {
  throw new Error("Expected a deno.lock file.");
}
