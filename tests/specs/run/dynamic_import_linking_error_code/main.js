// A missing named export is a spec-defined `SyntaxError` (ES InitializeEnvironment,
// step 7.c.ii) and must NOT carry a host-defined `code` property.
await import("./bad_export.js").catch((e) => {
  console.log("linking error name:", e.name);
  console.log("linking error has code:", "code" in e);
});

// A genuinely missing module is a host resolution error and should still
// report `ERR_MODULE_NOT_FOUND`.
await import("./does_not_exist.js").catch((e) => {
  console.log("missing module code:", e.code);
});
