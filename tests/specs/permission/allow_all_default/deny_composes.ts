// --deny-net composes on top of the allow-all default (deny wins): net is
// denied while read stays granted.
let net;
try {
  await fetch("http://localhost/");
  net = "UNEXPECTEDLY ALLOWED";
} catch (err) {
  net = err.name;
}
const read = (await Deno.permissions.query({ name: "read" })).state;
console.log(`net: ${net}, read: ${read}`);
