const content = Deno.readTextFileSync("./invalid_module.js");
console.log(content.includes("non-existent-package") ? "embedded" : "missing");
