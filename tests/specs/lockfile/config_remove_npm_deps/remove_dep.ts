const arg = Deno.args[0];
const filePath = import.meta.dirname + "/deno.json";
const denoJson = JSON.parse(Deno.readTextFileSync(filePath));
if (!(arg in denoJson.imports)) {
  throw new Error("Not found: " + arg);
}
delete denoJson.imports[arg];
Deno.writeTextFileSync(filePath, JSON.stringify(denoJson));
