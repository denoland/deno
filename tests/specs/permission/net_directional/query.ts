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

// Legacy name: "net" operates on the legacy `--allow-net` field only,
// not on the directional fields. With only `--allow-net-listen` set, the
// legacy field is empty, so this returns prompt.
const legacy = await Deno.permissions.query({
  name: "net",
  host: listenAddr,
});
console.log(`net ${listenAddr}: ${legacy.state}`);
