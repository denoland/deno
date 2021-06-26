console.log("Log from Deno");
console.warn("Warn from Deno");
console.error("Error from Deno");
console.table(Deno.resources());

// keep process alive
setInterval(() => {
}, 3000);
