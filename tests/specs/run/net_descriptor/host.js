const hosts = [
  "api.notion.com.",
  "api.notion.com.:80",
  "www.google.com",
];

for (const host of hosts) {
  Deno.connect({ hostname: host, port: 443 });
}
