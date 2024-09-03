async function checkAndPrintPermissions() {
  const readPerm = await Deno.permissions.query({ name: "read" });
  console.log(`Read permission: ${readPerm.state}`);

  const writePerm = await Deno.permissions.query({ name: "write" });
  console.log(`Write permission: ${writePerm.state}`);

  const netPerm = await Deno.permissions.query({ name: "net" });
  console.log(`Network permission: ${netPerm.state}`);

  const envPerm = await Deno.permissions.query({ name: "env" });
  console.log(`Environment permission: ${envPerm.state}`);

  const runPerm = await Deno.permissions.query({ name: "run" });
  console.log(`Run permission: ${runPerm.state}`);

  const ffiPerm = await Deno.permissions.query({ name: "ffi" });
  console.log(`FFI permission: ${ffiPerm.state}`);
}

checkAndPrintPermissions();
