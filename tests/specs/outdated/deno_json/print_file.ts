const file = Deno.args[0];
console.log(Deno.readTextFileSync(file).trim());
