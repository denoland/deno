const hosts = [
  "api.notion.com.",
  "www.google.com",
];

for (const host of hosts) {
  Deno.connect({ hostname: host, port: 443 });
}
