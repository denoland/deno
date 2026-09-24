// Regression test: --deny-net=127.0.0.1 must block binding a listener via a
// hostname that the OS resolver maps to the denied IP. The raw
// `process.binding("tcp_wrap")` API skips the `node:net` JS glue that
// pre-resolves the hostname, so the check has to happen after resolution.
// Both loopback families are denied so localhost works regardless of which
// address the resolver returns first.

// deno-lint-ignore no-explicit-any
const { TCP } = (process as any).binding("tcp_wrap");

const SERVER = 1;

function tryBind(name: string, address: string, bind6 = false) {
  const handle = new TCP(SERVER);
  try {
    const err = bind6 ? handle.bind6(address, 0, 0) : handle.bind(address, 0);
    console.log(`FAIL: ${name} was not denied (err=${err})`);
  } catch (error) {
    if (!(error instanceof Deno.errors.NotCapable)) throw error;
    console.log(`PASS: ${name} denied`);
  } finally {
    handle.close();
  }
}

if (Deno.args[0] === "numeric") {
  // Decimal and hex representations of 127.0.0.1 resolve on Unix. Windows
  // returns a resolver error before reaching the post-resolution check.
  tryBind("numeric 2130706433", "2130706433");
  tryBind("hex 0x7f000001", "0x7f000001");
  tryBind("bind6 numeric 2130706433", "2130706433", true);
  tryBind("bind6 hex 0x7f000001", "0x7f000001", true);
} else {
  tryBind("direct 127.0.0.1", "127.0.0.1");
  tryBind("bind6 ::1", "::1", true);

  // localhost passes the pre-resolution check and exercises the separate
  // post-resolution checks in bind and bind6 on every platform.
  tryBind("localhost", "localhost");
  tryBind("bind6 localhost", "localhost", true);
}
