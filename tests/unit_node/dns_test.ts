// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, assertThrows, fail } from "@std/assert";
import dns, { getDefaultResultOrder, lookupService } from "node:dns";
import dnsPromises, {
  getDefaultResultOrder as getDefaultResultOrderPromise,
  lookup as lookupPromise,
  lookupService as lookupServicePromise,
} from "node:dns/promises";
import { ErrnoException } from "ext:deno_node/_global.d.ts";

interface LookupServiceResult {
  hostname: string;
  service: string;
}

const address = "8.8.8.8";
const port = 80;

Deno.test("lookupService with callback", async () => {
  // Named import
  const result = await new Promise<LookupServiceResult>(
    (resolve, reject) => {
      lookupService(address, port, (err, hostname, service) => {
        if (err) reject(err);
        resolve({ hostname, service });
      });
    },
  );
  assertEquals(typeof result.hostname, "string");
  assertEquals(typeof result.service, "string");

  // Default import
  const defaultImportResult = await new Promise<LookupServiceResult>(
    (resolve, reject) => {
      dns.lookupService(address, port, (err, hostname, service) => {
        if (err) reject(err);
        resolve({ hostname, service });
      });
    },
  );
  assertEquals(typeof defaultImportResult.hostname, "string");
  assertEquals(typeof defaultImportResult.service, "string");
});

// Regression test for https://github.com/denoland/deno/issues/36537
// `dns.lookupService` must accept a numeric string port like Node.js instead
// of throwing `TypeError: expected i32`.
Deno.test("[node/dns] lookupService accepts a string port", async () => {
  const result = await new Promise<LookupServiceResult>(
    (resolve, reject) => {
      // deno-lint-ignore no-explicit-any
      lookupService("127.0.0.1", "80" as any, (err, hostname, service) => {
        if (err) reject(err);
        else resolve({ hostname, service });
      });
    },
  );
  assertEquals(typeof result.hostname, "string");
  assertEquals(typeof result.service, "string");
});

// Regression test for https://github.com/denoland/deno/issues/36518
// `Resolver.setLocalAddress` must not throw ERR_NOT_IMPLEMENTED; Node accepts
// it and returns undefined.
Deno.test("[node/dns] Resolver.setLocalAddress does not throw", () => {
  const resolver = new dns.promises.Resolver();
  resolver.setLocalAddress("0.0.0.0", "::");
  // The two addresses may be given in either order (one IPv4, one IPv6).
  resolver.setLocalAddress("::", "0.0.0.0");
  // Callable with only the IPv4 argument too.
  resolver.setLocalAddress("0.0.0.0");
  // ... but the second argument must be the *other* family, and an invalid
  // address is rejected, matching Node's c-ares `SetLocalAddress`.
  assertThrows(() => resolver.setLocalAddress("::1", "::1"));
  assertThrows(() => resolver.setLocalAddress("127.0.0.1", "127.0.0.1"));
  assertThrows(() => resolver.setLocalAddress("bad"));
});

// Regression test for https://github.com/denoland/deno/issues/36516
// `dns.resolveX` with a malformed hostname (e.g. an empty label) must report
// `EBADNAME` with `errno: undefined`, matching Node.js/c-ares, instead of
// flattening the error to `UNKNOWN` (errno -4094). The bad name is rejected
// while parsing the query, so this does not depend on network access.
Deno.test("[node/dns] resolve of a malformed hostname reports EBADNAME", async () => {
  const err = await new Promise<ErrnoException>((resolve) => {
    dns.resolve4("example..com", (err) => {
      resolve(err as unknown as ErrnoException);
    });
  });
  assert(err, "expected an error for a malformed hostname");
  assertEquals(err.code, "EBADNAME");
  assertEquals(err.errno, undefined);
  assertEquals(err.syscall, "queryA");
  assertEquals(err.hostname, "example..com");
});

Deno.test("lookupService promise", async () => {
  // Named import
  const result = await lookupServicePromise(address, port);
  assertEquals(typeof result.hostname, "string");
  assertEquals(typeof result.service, "string");

  // Default import
  const defaultImportResult = await dnsPromises.lookupService(
    address,
    port,
  );
  assertEquals(typeof defaultImportResult.hostname, "string");
  assertEquals(typeof defaultImportResult.service, "string");
});

Deno.test("[node/dns] lookupService accepts string ports", async () => {
  const stringPort = "80" as unknown as number;

  const callbackResult = await new Promise<LookupServiceResult>(
    (resolve, reject) => {
      lookupService("127.0.0.1", stringPort, (err, hostname, service) => {
        if (err) reject(err);
        else resolve({ hostname, service });
      });
    },
  );
  assertEquals(typeof callbackResult.hostname, "string");
  assertEquals(typeof callbackResult.service, "string");

  const promiseResult = await lookupServicePromise("127.0.0.1", stringPort);
  assertEquals(typeof promiseResult.hostname, "string");
  assertEquals(typeof promiseResult.service, "string");
});

Deno.test("lookupService not found", async () => {
  const address = "10.0.0.0";

  // Promise
  try {
    await lookupServicePromise(address, port);
    fail();
  } catch (err) {
    assertEquals(
      (err as ErrnoException).message,
      "getnameinfo ENOTFOUND 10.0.0.0",
    );
    assertEquals((err as ErrnoException).code, "ENOTFOUND");
    assertEquals((err as ErrnoException).syscall, "getnameinfo");
  }

  // Callback
  await new Promise<void>(
    (resolve, reject) => {
      dns.lookupService(address, port, (err) => {
        if (err) reject(err);
        resolve();
      });
    },
  ).then(() => fail(), (err) => {
    assertEquals(
      (err as ErrnoException).message,
      "getnameinfo ENOTFOUND 10.0.0.0",
    );
    assertEquals((err as ErrnoException).code, "ENOTFOUND");
    assertEquals((err as ErrnoException).syscall, "getnameinfo");
  });
});

Deno.test("[node/dns] getDefaultResultOrder returns valid order", () => {
  // Named export from dns
  const order = getDefaultResultOrder();
  assertEquals(typeof order, "string");
  assert(
    ["ipv4first", "ipv6first", "verbatim"].includes(order),
    `unexpected order: ${order}`,
  );

  // Default export from dns
  assertEquals(dns.getDefaultResultOrder(), order);

  // dns/promises named export
  assertEquals(getDefaultResultOrderPromise(), order);

  // dns.promises
  assertEquals(dns.promises.getDefaultResultOrder(), order);

  // dnsPromises default export
  assertEquals(dnsPromises.getDefaultResultOrder(), order);
});

Deno.test("[node/dns] getDefaultResultOrder reflects setDefaultResultOrder", () => {
  const original = dns.getDefaultResultOrder();
  try {
    dns.setDefaultResultOrder("ipv4first");
    assertEquals(dns.getDefaultResultOrder(), "ipv4first");

    dns.setDefaultResultOrder("verbatim");
    assertEquals(dns.getDefaultResultOrder(), "verbatim");
  } finally {
    // Restore original
    dns.setDefaultResultOrder(original);
  }
});

Deno.test("[node/dns] lookup accepts string family values", async () => {
  const ipv4Result = await lookupPromise("localhost", { family: "IPv4" });
  assertEquals(ipv4Result.family, 4);

  const ipv6Result = await lookupPromise("localhost", { family: "IPv6" });
  assertEquals(ipv6Result.family, 6);
});

// Regression test for https://github.com/denoland/deno/issues/25927
// `dns.lookup` must consult the operating system resolver (which reads the
// hosts file, e.g. /etc/hosts) rather than querying DNS servers directly.
// `localhost` is present in the hosts file on every supported platform and is
// not resolvable through public DNS, so it exercises that path.
Deno.test("[node/dns] lookup uses the system resolver / hosts file", async () => {
  const { address, family } = await new Promise<
    { address: string; family: number }
  >((resolve, reject) => {
    dns.lookup("localhost", (err, address, family) => {
      if (err) reject(err);
      else resolve({ address, family });
    });
  });
  assert(
    address === "127.0.0.1" || address === "::1",
    `unexpected address for localhost: ${address}`,
  );
  assert(family === 4 || family === 6, `unexpected family: ${family}`);

  const all = await lookupPromise("localhost", { all: true });
  assert(Array.isArray(all) && all.length > 0, "expected at least one address");
  assert(
    all.every(({ address }) => address === "127.0.0.1" || address === "::1"),
    `unexpected addresses for localhost: ${JSON.stringify(all)}`,
  );
});

// Regression test for https://github.com/denoland/deno/issues/34801
// `dns.lookup` must follow Node.js behavior when `hostname` is falsy
// (undefined / null / empty string): the callback is invoked with
// (null, null, family) instead of throwing synchronously.
Deno.test("[node/dns] lookup with falsy hostname invokes callback", async () => {
  for (const hostname of [undefined, null, ""]) {
    const result = await new Promise<
      { error: unknown; address: unknown; family: unknown }
    >((resolve) => {
      // deno-lint-ignore no-explicit-any
      dns.lookup(hostname as any, (error, address, family) => {
        resolve({ error, address, family });
      });
    });
    assertEquals(result, { error: null, address: null, family: 4 });
  }

  // family argument is honored when 6.
  const result6 = await new Promise<
    { error: unknown; address: unknown; family: unknown }
  >((resolve) => {
    dns.lookup(
      // deno-lint-ignore no-explicit-any
      undefined as any,
      6,
      (error, address, family) => resolve({ error, address, family }),
    );
  });
  assertEquals(result6, { error: null, address: null, family: 6 });

  // options.family = 6
  const resultOpt6 = await new Promise<
    { error: unknown; address: unknown; family: unknown }
  >((resolve) => {
    dns.lookup(
      // deno-lint-ignore no-explicit-any
      undefined as any,
      { family: 6 },
      (error, address, family) => resolve({ error, address, family }),
    );
  });
  assertEquals(resultOpt6, { error: null, address: null, family: 6 });

  // options.all = true returns an empty array via the callback.
  const resultAll = await new Promise<
    { error: unknown; addresses: unknown }
  >((resolve) => {
    dns.lookup(
      // deno-lint-ignore no-explicit-any
      undefined as any,
      { all: true },
      // deno-lint-ignore no-explicit-any
      (error, addresses: any) => resolve({ error, addresses }),
    );
  });
  assertEquals(resultAll, { error: null, addresses: [] });
});

Deno.test("[node/dns] promises.lookup with falsy hostname resolves", async () => {
  for (const hostname of [undefined, null, ""]) {
    // deno-lint-ignore no-explicit-any
    const result = await lookupPromise(hostname as any);
    assertEquals(result as unknown, { address: null, family: 4 });
  }

  // deno-lint-ignore no-explicit-any
  const result6 = await lookupPromise(undefined as any, { family: 6 });
  assertEquals(result6 as unknown, { address: null, family: 6 });

  // deno-lint-ignore no-explicit-any
  const resultAll = await lookupPromise(undefined as any, { all: true });
  assertEquals(resultAll, []);
});

// Regression test for https://github.com/denoland/deno/issues/25927
// A failed `dns.lookup` must report the real libuv error code and errno
// (ENOTFOUND / -3008) like Node.js, instead of flattening every failure to
// EAI_NODATA (-3007).
Deno.test("[node/dns] lookup of a missing host reports ENOTFOUND", async () => {
  const err = await new Promise<ErrnoException>((resolve) => {
    dns.lookup("nonexistent-host.invalid", (err) => {
      resolve(err as unknown as ErrnoException);
    });
  });
  assert(err, "expected an error for an unresolvable host");
  assertEquals(err.code, "ENOTFOUND");
  assertEquals(err.errno, -3008);
  assertEquals(err.syscall, "getaddrinfo");
  assertEquals(err.hostname, "nonexistent-host.invalid");
});

function assertEmptyHostnameResolveError(
  error: unknown,
  syscall: "queryA" | "queryMx",
) {
  assert(error instanceof Error);

  const dnsError = error as ErrnoException;
  assertEquals(dnsError.name, "Error");
  assertEquals(dnsError.message, `${syscall} ENODATA`);
  assertEquals(dnsError.code, "ENODATA");
  assertEquals(dnsError.errno, undefined);
  assertEquals(dnsError.syscall, syscall);
  assertEquals(dnsError.hostname, undefined);
}

Deno.test(
  "[node/dns] resolve empty hostname callback reports ENODATA",
  async () => {
    const error = await new Promise<unknown>((resolve) => {
      dns.resolve("", "A", (error) => resolve(error));
    });

    assertEmptyHostnameResolveError(error, "queryA");
  },
);

Deno.test(
  "[node/dns] promises.resolve empty hostname reports ENODATA",
  async () => {
    const error = await dnsPromises.resolve("", "A").then(
      () => undefined,
      (error) => error,
    );

    assertEmptyHostnameResolveError(error, "queryA");
  },
);

Deno.test(
  "[node/dns] promises.resolve empty hostname as MX reports ENODATA",
  async () => {
    const error = await dnsPromises.resolve("", "MX").then(
      () => undefined,
      (error) => error,
    );

    assertEmptyHostnameResolveError(error, "queryMx");
  },
);

Deno.test(
  "[node/dns] resolve of a missing host reports ENOTFOUND",
  async () => {
    const error = await dnsPromises.resolve(
      "nonexistent-host.invalid",
      "A",
    ).then(
      () => undefined,
      (error) => error,
    );

    assert(error instanceof Error);
    const dnsError = error as ErrnoException;
    assertEquals(
      dnsError.message,
      "queryA ENOTFOUND nonexistent-host.invalid",
    );
    assertEquals(dnsError.code, "ENOTFOUND");
    assertEquals(dnsError.syscall, "queryA");
    assertEquals(dnsError.hostname, "nonexistent-host.invalid");
  },
);

Deno.test(
  "[node/dns] custom resolver preserves NXDOMAIN for empty hostname",
  async () => {
    const server = Deno.listenDatagram({
      transport: "udp",
      hostname: "127.0.0.1",
      port: 0,
    });
    const { port } = server.addr as Deno.NetAddr;
    const resolver = new dnsPromises.Resolver();
    resolver.setServers([`127.0.0.1:${port}`]);

    const responseSent = (async () => {
      const [request, remoteAddress] = await server.receive();
      const response = request.slice();
      response[2] = 0x81;
      response[3] = 0x83; // Standard recursive response with NXDOMAIN.
      response.fill(0, 6, 12);
      await server.send(response, remoteAddress);
    })();

    try {
      const error = await resolver.resolve("", "A").then(
        () => undefined,
        (error) => error,
      );
      await responseSent;

      assert(error instanceof Error);
      const dnsError = error as ErrnoException;
      assertEquals(dnsError.message, "queryA ENOTFOUND");
      assertEquals(dnsError.code, "ENOTFOUND");
      assertEquals(dnsError.syscall, "queryA");
      assertEquals(dnsError.hostname, undefined);
    } finally {
      server.close();
    }
  },
);
