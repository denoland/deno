console.log(performance.now() % 2 !== 0);
Deno.revokePermission("hrtime");
console.log(performance.now() % 2 === 0);
