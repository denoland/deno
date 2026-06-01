const p = JSON.parse(Deno.readTextFileSync("package.json"));
console.log(p.dependencies["@denotest/with-vuln1"]);
