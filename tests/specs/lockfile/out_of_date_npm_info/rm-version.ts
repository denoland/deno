const registryPath = Deno.args[0].trim();
const version = Deno.args[1].trim();

const registryJson = JSON.parse(Deno.readTextFileSync(registryPath));

delete registryJson.versions[version];

if (registryJson["dist-tags"]["latest"] === version) {
  const latestVersion = Object.keys(registryJson.versions).sort()[0];
  registryJson["dist-tags"]["latest"] = latestVersion;
}
const registryJsonString = JSON.stringify(registryJson, null, 2);
Deno.writeTextFileSync(registryPath, registryJsonString);
console.log(registryJsonString);
