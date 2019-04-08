console.log(performance.now() % 2 !== 0);
Deno.revokePermission("highPrecision");
console.log(performance.now() % 2 === 0);
