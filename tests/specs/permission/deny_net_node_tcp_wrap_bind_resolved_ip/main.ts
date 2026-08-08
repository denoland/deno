// Regression test: --deny-net=127.0.0.1 must block binding a listener via a
// hostname that the OS resolver maps to the denied IP. The raw
// `process.binding("tcp_wrap")` API skips the `node:net` JS glue that
// pre-resolves the hostname, so the check has to happen after resolution.
// e.g. 2130706433 is the decimal representation of 127.0.0.1.

// deno-lint-ignore no-explicit-any
const { TCP } = (process as any).binding("tcp_wrap");

const SERVER = 1;

function tryBind(name: string, address: string, bind6 = false) {
  const handle = new TCP(SERVER);
  try {
    const err = bind6 ? handle.bind6(address, 0, 0) : handle.bind(address, 0);
    console.log(`FAIL: ${name} was not denied (err=${err})`);
  } catch {
    console.log(`PASS: ${name} denied`);
  }
}

// Binding to 127.0.0.1 directly — should be denied.
tryBind("direct 127.0.0.1", "127.0.0.1");

// Binding via decimal numeric hostname 2130706433 — should also be denied.
tryBind("numeric 2130706433", "2130706433");

// Binding via 0x7f000001 (hex form) — should also be denied.
tryBind("hex 0x7f000001", "0x7f000001");

// Same via bind6, which has its own copy of the check.
tryBind("bind6 ::1", "::1", true);
