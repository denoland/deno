import { assert } from "./test_util.ts";

Deno.test(
  { name: "Deno.networkInterfaces", permissions: { env: true } },
  () => {
    const networkInterfaces = Deno.networkInterfaces();
    assert(Array.isArray(networkInterfaces));
    assert(networkInterfaces.length > 0);
    for (
      const { name, family, address, netmask, scopeid, cidr, mac }
        of networkInterfaces
    ) {
      assert(typeof name === "string");
      assert(family === "IPv4" || family === "IPv6");
      assert(typeof address === "string");
      assert(typeof netmask === "string");
      assert(
        (family === "IPv6" && typeof scopeid === "number") ||
          (family === "IPv4" && scopeid === null),
      );
      assert(typeof cidr === "string");
      assert(typeof mac === "string");
    }
  },
);
