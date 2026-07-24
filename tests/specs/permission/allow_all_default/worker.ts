// Inherits the allow-all default from the parent: every capability is granted.
const read = (await Deno.permissions.query({ name: "read" })).state;
const net = (await Deno.permissions.query({ name: "net" })).state;
self.postMessage(`worker read: ${read}, worker net: ${net}`);
