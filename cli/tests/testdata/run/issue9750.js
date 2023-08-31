// Run without permissions.
const buf = new Uint8Array(1);
console.log("Enter 'yy':");
await Deno.stdin.read(buf);
await Deno.permissions.request({ "name": "env" });
console.log("\n\nOwned", Deno.env.get("SECRET"));
