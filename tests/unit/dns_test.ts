// Copyright 2018-2025 the Deno authors. MIT license.

import { resolve4, resolve6 } from "node:dns/promises";
import { assertEquals } from "@std/assert/equals";

Deno.test({
  name: "Dns resolving for ttl values, A and AAAA records",
  async fn() {
    const ARecord = "34.120.54.55";
    const AAAARecord = "2600:1901::6d85::";

    const ARes1 = await Deno.resolveDns("deno.com", "A", { ttl: true });
    const ARes2 = await resolve4("deno.com", { ttl: true });

    assertEquals(ARes1[0].data, ARecord);
    assertEquals(ARes2[0].address, ARecord);

    const AAAARes1 = await Deno.resolveDns("deno.com", "AAAA", { ttl: true });
    const AAAARes2 = await resolve6("deno.com", { ttl: true });

    assertEquals(AAAARes1[0].data, AAAARecord);
    assertEquals(AAAARes2[0].address, AAAARecord);
  },
});
