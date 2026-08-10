// Remove the `time` field and the `_deno.packumentFormat` marker from the
// cached packument (registry.json), accurately simulating a cache written by
// an older Deno that predates full-packument metadata.
const path = Deno.args[0] +
  "/npm/localhost_4260/@denotest/esm-basic/registry.json";
const text = Deno.readTextFileSync(path);
const json = JSON.parse(text);
delete json.time;
delete json._deno?.packumentFormat;
Deno.writeTextFileSync(path, JSON.stringify(json));
