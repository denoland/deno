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

console.log("A:", a);
console.log("AAAA:", aaaa);
console.log("ANAME:", aname);
console.log("CNAME:", cname);
console.log("MX:", mx);
console.log("PTR:", ptr);
console.log("SRV:", srv);
console.log("TXT:", txt);
