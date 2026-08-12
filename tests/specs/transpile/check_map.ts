const m = JSON.parse(Deno.readTextFileSync("out.js.map"));
// Verify source map version and that "file" points to the output file
console.log(m.version);
console.log(m.file);
