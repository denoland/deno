// Copyright 2018-2025 the Deno authors. MIT license.
import dns2 from "npm:dns2@2.1.0";

Deno.test("EDNS0 enabled", async () => {
  // With EDNS0 enabled, Deno.resolveDns can handle 44 A records.
  const NUM_RECORD = 44;
  const { Packet } = dns2;
  const server = dns2.createServer({
    udp: true,
    // deno-lint-ignore no-explicit-any
    handle(request: any, send: any) {
      const response = Packet.createResponseFromRequest(request);
      const { name } = request.questions[0];
      for (const i of [...Array(NUM_RECORD).keys()]) {
        response.answers.push({
          name,
          type: Packet.TYPE.A,
          class: Packet.CLASS.IN,
          ttl: 300,
          address: "1.2.3." + i,
        });
      }
      send(response);
    },
  });
  const { udp: { port } } = await server.listen({
    udp: { port: 0, address: "127.0.0.1", type: "udp4" },
  });
  const addr = await Deno.resolveDns("example.com", "A", {
    nameServer: { ipAddr: "127.0.0.1", port },
  });
  if (addr.length !== NUM_RECORD) {
    throw new Error(`Expected ${NUM_RECORD} records, got ${addr.length}`);
  }
  await server.close();
});
