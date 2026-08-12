const arch = Deno.args[0];
const notArch = Deno.args[1];
const pkg = `@denotest+multiple-arches-win32-${arch}@1.0.0`;
try {
  Deno.statSync(`node_modules/.deno/${pkg}`);
  console.log(`${arch}: FOUND`);
} catch {
  console.log(`${arch}: NOT FOUND`);
}
if (notArch) {
  const notPkg = `@denotest+multiple-arches-win32-${notArch}@1.0.0`;
  try {
    Deno.statSync(`node_modules/.deno/${notPkg}`);
    console.log(`${notArch}: FOUND (unexpected)`);
  } catch {
    console.log(`${notArch}: NOT FOUND (expected)`);
  }
}
