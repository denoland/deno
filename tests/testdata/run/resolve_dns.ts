const nameServer = { nameServer: { ipAddr: "127.0.0.1", port: 4553 } };

const [a, aaaa, aname, caa, cname, mx, naptr, ns, ptr, soa, srv, txt] =
  await Promise
    .all([
      Deno.resolveDns("www.example.com", "A", nameServer),
      Deno.resolveDns("www.example.com", "AAAA", nameServer),
      Deno.resolveDns("www.example.com", "ANAME", nameServer),
      Deno.resolveDns("example.com", "CAA", nameServer),
      Deno.resolveDns("alias.example.com", "CNAME", nameServer),
      Deno.resolveDns("example.com", "MX", nameServer),
      Deno.resolveDns("example.com", "NAPTR", nameServer),
      Deno.resolveDns("example.com", "NS", nameServer),
      Deno.resolveDns("1.2.3.4.IN-ADDR.ARPA.", "PTR", nameServer),
      Deno.resolveDns("example.com", "SOA", nameServer),
      Deno.resolveDns("_service._tcp.example.com", "SRV", nameServer),
      Deno.resolveDns("example.com", "TXT", nameServer),
    ]);

console.log("A");
console.log(JSON.stringify(a));

console.log("AAAA");
console.log(JSON.stringify(aaaa));

console.log("ANAME");
console.log(JSON.stringify(aname));

console.log("CAA");
console.log(JSON.stringify(caa));

console.log("CNAME");
console.log(JSON.stringify(cname));

console.log("MX");
console.log(JSON.stringify(mx));

console.log("NAPTR");
console.log(JSON.stringify(naptr));

console.log("NS");
console.log(JSON.stringify(ns));

console.log("PTR");
console.log(JSON.stringify(ptr));

console.log("SOA");
console.log(JSON.stringify(soa));

console.log("SRV");
console.log(JSON.stringify(srv));

console.log("TXT");
console.log(JSON.stringify(txt));

try {
  await Deno.resolveDns("not-found-example.com", "A", nameServer);
} catch (e) {
  console.log(
    `Error ${
      e instanceof Error ? e.name : "[non-error]"
    } thrown for not-found-example.com`,
  );
}

try {
  // @ts-ignore testing invalid overloads
  await Deno.resolveDns("example.com", "SSHFP", nameServer);
} catch (e) {
  console.log((e as Error).message);
}

try {
  const ac = new AbortController();
  queueMicrotask(() => ac.abort());
  await Deno.resolveDns("www.example.com", "A", {
    ...nameServer,
    signal: ac.signal,
  });
} catch (e) {
  console.log((e as Error).name);
}

try {
  const ac = new AbortController();
  ac.abort();
  await Deno.resolveDns("www.example.com", "A", {
    ...nameServer,
    signal: ac.signal,
  });
} catch (e) {
  console.log((e as Error).name);
}
