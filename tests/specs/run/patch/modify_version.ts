const data = JSON.parse(Deno.readTextFileSync("./add/deno.json"));
data.version = "2.0.0";
Deno.writeTextFileSync("./add/deno.json", JSON.stringify(data, null, 2));
