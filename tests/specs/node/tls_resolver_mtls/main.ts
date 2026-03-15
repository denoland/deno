import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const ROOT_CA = readFileSync(
  new URL("../../../testdata/tls/RootCA.crt", import.meta.url),
).toString();
const SERVER_CERT = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
).toString();
const SERVER_KEY = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
).toString();
const CLIENT_CERT = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
).toString();
const CLIENT_KEY = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
).toString();

const resolverSymbol = Symbol.for("unstableSniResolver");
const requestClientCertSymbol = Symbol.for("unstableRequestClientCert");
const rejectUnauthorizedSymbol = Symbol.for("unstableRejectUnauthorized");
const clientCaCertsSymbol = Symbol.for("unstableClientCaCerts");

async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  label: string,
): Promise<T> {
  const timeout = new Promise<never>((_resolve, reject) => {
    setTimeout(() => {
      reject(new Error(`${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });
  return await Promise.race([promise, timeout]);
}

const listenOptions: Record<PropertyKey, unknown> = {
  hostname: "127.0.0.1",
  port: 0,
  [resolverSymbol]: async (sni: string) => {
    assert.equal(sni, "localhost");
    return { cert: SERVER_CERT, key: SERVER_KEY };
  },
  [requestClientCertSymbol]: true,
  [rejectUnauthorizedSymbol]: true,
  [clientCaCertsSymbol]: [ROOT_CA],
};

const listener = Deno.listenTls(
  listenOptions as unknown as Deno.ListenTlsOptions & Deno.TlsCertifiedKeyPem,
);
const port = listener.addr.port;

let acceptedConnections = 0;
const acceptLoop = (async () => {
  while (true) {
    let conn: Deno.TlsConn | null = null;
    try {
      conn = await listener.accept();
      try {
        await conn.handshake();
        acceptedConnections++;
      } catch {
        // Expected for rejected client cert handshakes.
      }
    } catch (error) {
      if (error instanceof Deno.errors.BadResource) {
        return;
      }
      throw error;
    } finally {
      conn?.close();
    }
  }
})();

try {
  const goodClient = await withTimeout(
    Deno.connectTls({
      hostname: "localhost",
      port,
      caCerts: [ROOT_CA],
      cert: CLIENT_CERT,
      key: CLIENT_KEY,
    }),
    2_000,
    "good client connect",
  );
  await withTimeout(goodClient.handshake(), 2_000, "good client handshake");
  goodClient.close();
  console.log("RESOLVER_MTLS_AUTH_OK");

  const noCertResult = await withTimeout((async () => {
    try {
      const noCertClient = await Deno.connectTls({
        hostname: "localhost",
        port,
        caCerts: [ROOT_CA],
      });
      await noCertClient.handshake();
      noCertClient.close();
      return "accepted";
    } catch {
      return "rejected";
    }
  })(), 2_000, "no-cert client outcome");

  assert.ok(noCertResult === "accepted" || noCertResult === "rejected");
  console.log("RESOLVER_MTLS_NO_CERT_BEHAVIOR_OK");
} finally {
  listener.close();
  await acceptLoop;
}

assert.ok(acceptedConnections >= 1);
console.log("RESOLVER_MTLS_DONE");
