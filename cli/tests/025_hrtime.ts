console.log(performance.now() % 2 !== 0);
Deno.permissions.revoke({ name: "hrtime" });
console.log(performance.now() % 2 === 0);
