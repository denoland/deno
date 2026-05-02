const file = Deno.args[0].trim();
const importKey = Deno.args[1].trim();
const newValue = Deno.args[2].trim();
const json = JSON.parse(Deno.readTextFileSync(file));
json["imports"][importKey] = newValue;
Deno.writeTextFileSync(file, JSON.stringify(json));
