const hosts = [
  "api.notion.com.",
  "api.notion.com.:80",
  "https://api.notion.com.:443",
  "https://api.notion.com.",
  "www.google.com",
];

for (const host of hosts) {
  Deno.connect({ hostname: host, port: 443 });
}
