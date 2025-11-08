// test_writeEarlyHints.mjs
import http from "node:http";

let callbackExecuted = false;

const server = http.createServer((req, res) => {
  // Test method exists
  if (typeof res.writeEarlyHints === "function") {
    console.log("✅ PASS: writeEarlyHints method exists");
  } else {
    console.log("❌ FAIL: writeEarlyHints method missing");
    res.end();
    return;
  }

  // Test method execution
  try {
    res.writeEarlyHints({
      "link": "</styles.css>; rel=preload; as=style",
    }, () => {
      callbackExecuted = true;
      console.log("✅ PASS: writeEarlyHints callback executed");
    });

    console.log("✅ PASS: writeEarlyHints executed without error");
  } catch (e) {
    console.log("❌ FAIL: writeEarlyHints threw error:", e.message);
  }

  res.writeHead(200);
  res.end("Hello World");

  setTimeout(() => {
    if (callbackExecuted) {
      console.log("✅ PASS: Callback was executed asynchronously");
    } else {
      console.log("❌ FAIL: Callback was not executed");
    }
    server.close();
  }, 100);
});

server.listen(8000, () => {
  // Make a request to trigger the handler
  const req = http.request("http://localhost:8000", () => {});
  req.end();
});
