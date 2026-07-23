// Regression test: node:dns.getServers() reads the host resolver
// configuration (the system nameservers, e.g. from /etc/resolv.conf) via
// op_net_get_system_dns_servers. That is host network configuration, the same
// sys-info class as Deno.networkInterfaces(), so it must require
// `--allow-sys`. Without it the call must throw NotCapable rather than leak
// the host's configured nameservers.

import dns from "node:dns";

try {
  dns.getServers();
  console.log("result: allowed");
} catch (e) {
  console.log(
    "result:",
    (e as Error).name === "NotCapable" ? "denied" : `other:${(e as Error).name}`,
  );
}
