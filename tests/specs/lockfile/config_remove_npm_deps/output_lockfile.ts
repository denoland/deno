const filePath = import.meta.dirname + "/deno.lock";
console.log(Deno.readTextFileSync(filePath).trim());
