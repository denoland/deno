// Test that sourcemap URLs don't exfiltrate data without permission
function throwError() {
  throw new Error("Error without sourcemap permission");
}

throwError();

//# sourceMappingURL=http://localhost:4545/run/sourcemap_external.js.map
