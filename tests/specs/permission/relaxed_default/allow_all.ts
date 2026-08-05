// -A allows everything, unchanged by the profile.
const net = await Deno.permissions.query({ name: "net" });
const write = await Deno.permissions.query({ name: "write" });
console.log(`net: ${net.state}`);
console.log(`write: ${write.state}`);
