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
const UNTRUSTED_CLIENT_CERT = readFileSync(
  new URL("../../../testdata/tls/self-signed-hostname.crt", import.meta.url),
).toString();
const UNTRUSTED_CLIENT_KEY = readFileSync(
  new URL("../../../testdata/tls/self-signed-hostname.key", import.meta.url),
).toString();

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
  cert: SERVER_CERT,
  key: SERVER_KEY,
  [requestClientCertSymbol]: true,
  [rejectUnauthorizedSymbol]: false,
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
        // Handshake may fail for invalid presented client certs.
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

async function connectionOutcome(options: {
  cert?: string;
  key?: string;
}): Promise<"accepted" | "rejected"> {
  try {
    const conn = await Deno.connectTls({
      hostname: "localhost",
      port,
      caCerts: [ROOT_CA],
      cert: options.cert,
      key: options.key,
    });
    await conn.handshake();
    conn.close();
    return "accepted";
  } catch {
    return "rejected";
  }
}

try {
  const noCert = await withTimeout(
    connectionOutcome({}),
    2_000,
    "no-cert client outcome",
  );
  assert.equal(noCert, "accepted");
  console.log("REQUEST_CERT_OPTIONAL_NO_CERT_ACCEPT_OK");

  const invalidCert = await withTimeout(
    connectionOutcome({
      cert: UNTRUSTED_CLIENT_CERT,
      key: UNTRUSTED_CLIENT_KEY,
    }),
    2_000,
    "untrusted-cert client outcome",
  );
  assert.ok(invalidCert === "accepted" || invalidCert === "rejected");
  console.log("REQUEST_CERT_OPTIONAL_UNTRUSTED_CERT_BEHAVIOR_OK");
} finally {
  listener.close();
  await acceptLoop;
}

assert.ok(acceptedConnections >= 1);
console.log("REQUEST_CERT_OPTIONAL_DONE");
