const m = JSON.parse(Deno.readTextFileSync("out.js.map"));
console.log(m.version);
