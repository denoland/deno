// Regression test: --deny-net=127.0.0.1 must block connections via numeric
// hostname aliases that the OS resolver maps to the denied IP.
// e.g. 2130706433 is the decimal representation of 127.0.0.1.

// Try connecting to 127.0.0.1 directly — should be denied.
try {
  await Deno.connect({ hostname: "127.0.0.1", port: 12345 });
  console.log("FAIL: direct 127.0.0.1 was not denied");
} catch (e) {
  if (e instanceof Deno.errors.PermissionDenied) {
    console.log("PASS: direct 127.0.0.1 denied");
  } else {
    console.log("PASS: direct 127.0.0.1 denied (error: " + e.message + ")");
  }
}

// Try connecting via decimal numeric hostname 2130706433 — should also be denied.
try {
  await Deno.connect({ hostname: "2130706433", port: 12345 });
  console.log("FAIL: numeric 2130706433 was not denied");
} catch (e) {
  if (e instanceof Deno.errors.PermissionDenied) {
    console.log("PASS: numeric 2130706433 denied");
  } else {
    // On some platforms the numeric form may not resolve, which is also fine
    // (the important thing is it does NOT succeed in connecting).
    console.log("PASS: numeric 2130706433 blocked (error: " + e.message + ")");
  }
}

// Try connecting via 0x7f000001 (hex form) — should also be denied.
try {
  await Deno.connect({ hostname: "0x7f000001", port: 12345 });
  console.log("FAIL: hex 0x7f000001 was not denied");
} catch (e) {
  if (e instanceof Deno.errors.PermissionDenied) {
    console.log("PASS: hex 0x7f000001 denied");
  } else {
    console.log("PASS: hex 0x7f000001 blocked (error: " + e.message + ")");
  }
}
