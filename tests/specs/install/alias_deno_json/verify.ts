const stat = Deno.statSync(new URL("./node_modules/alias", import.meta.url));
console.log(stat.isDirectory);
