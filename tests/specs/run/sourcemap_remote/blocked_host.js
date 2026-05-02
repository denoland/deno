// Test that sourcemap URLs from non-allowed hosts are blocked
function throwError() {
  throw new Error("Error with blocked sourcemap host");
}

throwError();

//# sourceMappingURL=http://evil.example.com/exfiltrate/sourcemap.js.map
