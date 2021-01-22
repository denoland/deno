const nameServer = { nameServer: { ipAddr: "127.0.0.1", port: 4553 } };

const [a, aaaa, aname, cname, mx, ptr, srv, txt] = await Promise.all([
  Deno.resolveDns("www.example.com", "A", nameServer),
  Deno.resolveDns("www.example.com", "AAAA", nameServer),
  Deno.resolveDns("www.example.com", "ANAME", nameServer),
  Deno.resolveDns("foo", "CNAME", nameServer),
  Deno.resolveDns("www.example.com", "MX", nameServer),
  Deno.resolveDns("5.6.7.8", "PTR", nameServer),
  Deno.resolveDns("_Service._TCP.example.com", "SRV", nameServer),
  Deno.resolveDns("www.example.com", "TXT", nameServer),
]);

console.log("A");
console.log(JSON.stringify(a));

console.log("AAAA");
console.log(JSON.stringify(aaaa));

console.log("ANAME");
console.log(JSON.stringify(aname));

console.log("CNAME");
console.log(JSON.stringify(cname));

console.log("MX");
console.log(JSON.stringify(mx));

console.log("PTR");
console.log(JSON.stringify(ptr));

console.log("SRV");
console.log(JSON.stringify(srv));

console.log("TXT");
console.log(JSON.stringify(txt));

try {
  await Deno.resolveDns("not-found-example.com", "A", nameServer);
} catch (e) {
  console.log("Error thrown for not-found-example.com");
}
