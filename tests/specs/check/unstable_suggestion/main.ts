/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />

// unstable apis removed here, so should error
Deno.listenDatagram({
  port: 3000,
  transport: "udp",
});
