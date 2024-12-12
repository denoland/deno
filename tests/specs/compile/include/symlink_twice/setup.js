Deno.writeTextFileSync("index.js", "console.log(1);");
Deno.symlinkSync("index.js", "link.js");
