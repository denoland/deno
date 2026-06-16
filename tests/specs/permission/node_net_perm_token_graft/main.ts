// Regression test for GHSA-fhjh-jqv7-m238.
//
// The NetPermToken produced by a DNS lookup must never be observable by user
// code, and a custom `net.connect` `lookup` must not be able to authorize a
// connection to an arbitrary IP. The built-in lookup must still let
// `--allow-net=<host>` authorize the IPs that host resolves to.
//
// This process is granted only `--allow-net=localhost` (resolves to 127.0.0.1).

import net from "node:net";
import dns from "node:dns";

const PORT = 45999;

// Grab whatever a user dns.lookup callback receives in the 4th argument slot.
function lookupToken(host: string): Promise<unknown> {
  return new Promise((resolve) =>
    dns.lookup(host, { family: 4 }, (_e, _a, _f, token) => resolve(token))
  );
}

// Attempt a node:net connection. With a custom `lookup`, the lookup decides the
// IP (and may try to inject a token); without one, the built-in lookup is used.
function connect(lookup?: unknown): Promise<string> {
  return new Promise((resolve) => {
    const socket = net.connect({
      host: "localhost",
      port: PORT,
      family: 4,
      lookup,
    });
    socket.on("connect", () => {
      socket.destroy();
      resolve("connected");
    });
    socket.on("error", (e: Error) => {
      const msg = `${e.name}: ${e.message}`;
      resolve(
        e.name === "NotCapable" || msg.includes("Requires net access")
          ? "perm-denied"
          : "other",
      );
    });
  });
}

// 1. dns.lookup must not hand a permission token to user callbacks.
const stolen = await lookupToken("localhost");
console.log(
  stolen === undefined
    ? "PASS: dns.lookup did not leak a perm token"
    : `FAIL: dns.lookup leaked ${typeof stolen}`,
);

// 2. A custom lookup cannot graft access to an unrelated IP, even when it tries
//    to pass something in the token slot.
const graft = await connect((_h, _o, cb) => cb(null, "127.0.0.2", 4, stolen));
console.log(
  graft === "perm-denied"
    ? "PASS: custom lookup to 127.0.0.2 denied"
    : `FAIL: custom lookup to 127.0.0.2 was ${graft}`,
);

// 3. The built-in lookup still lets --allow-net=localhost authorize the IPs
//    localhost resolves to (127.0.0.1). The socket then fails to connect with
//    no server present, which is past the permission check and therefore fine.
const legit = await connect();
console.log(
  legit !== "perm-denied"
    ? "PASS: built-in lookup authorized localhost's resolved IP"
    : "FAIL: built-in lookup denied localhost",
);
