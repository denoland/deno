// Verify Deno.permissions.query against the new directional descriptors.

const listenAddr = "127.0.0.1:0";
const connectAddr = "deno.land:443";

const listenGranted = await Deno.permissions.query({
  name: "net-listen",
  host: listenAddr,
});
console.log(`net-listen ${listenAddr}: ${listenGranted.state}`);

const connectPrompt = await Deno.permissions.query({
  name: "net-connect",
  host: connectAddr,
});
console.log(`net-connect ${connectAddr}: ${connectPrompt.state}`);

// Legacy name: "net" with the listen-only grant should be Prompt because
// only one direction is granted (the conjunction of the two directional
// states).
const legacy = await Deno.permissions.query({
  name: "net",
  host: listenAddr,
});
console.log(`net ${listenAddr}: ${legacy.state}`);
