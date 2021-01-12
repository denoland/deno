const RecordA = await Deno.resolveDns(
  "www.example.com",
  "A",
  { nameServer: { ipAddr: "127.0.0.1", port: 4553 } },
);

console.log(RecordA);

const RecordAAAA = await Deno.resolveDns(
  "www.example.com",
  "AAAA",
  { nameServer: { ipAddr: "127.0.0.1", port: 4553 } },
);

console.log(RecordAAAA);

// TODO(magurotuna): add tests for other record types
