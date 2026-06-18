// Regression test: in directional mode `Deno.permissions.query` must agree
// with enforcement. A host that is hard-denied via `--deny-net-connect` must
// report "denied", not "prompt" (the legacy `net` permission is empty in
// directional mode and must not mask the directional deny).
const deniedConnect = await Deno.permissions.query({
  name: "net-connect",
  host: "evil.com",
});
console.log(`net-connect evil.com: ${deniedConnect.state}`);

// A connect host that is neither allowed nor denied still prompts.
const otherConnect = await Deno.permissions.query({
  name: "net-connect",
  host: "deno.land",
});
console.log(`net-connect deno.land: ${otherConnect.state}`);

// The deny is connect-only; the listen direction is unaffected.
const listen = await Deno.permissions.query({
  name: "net-listen",
  host: "evil.com",
});
console.log(`net-listen evil.com: ${listen.state}`);
