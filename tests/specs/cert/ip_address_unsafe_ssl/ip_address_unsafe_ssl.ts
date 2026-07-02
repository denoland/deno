// Use the local test HTTPS server (port 5545) via its IP address.
// The cert is issued for "localhost", not 127.0.0.1, so this exercises
// --unsafely-ignore-certificate-errors with an IP address.
const r = await fetch("https://127.0.0.1:5545/echo.ts");
console.log(r.status);
