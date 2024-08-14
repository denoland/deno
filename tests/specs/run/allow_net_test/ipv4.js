const hosts = [
  "142.251.41.4",
  "142.251.41.4:443",
];

for (const host of hosts) {
  Deno.connect({ hostname: host, port: 443 });
}
