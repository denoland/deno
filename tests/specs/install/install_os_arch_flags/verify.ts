const arch = Deno.args[0];
const pkg = `@denotest+multiple-arches-win32-${arch}@1.0.0`;
try {
  Deno.statSync(`node_modules/.deno/${pkg}`);
  console.log(`${arch}: FOUND`);
} catch {
  console.log(`${arch}: NOT FOUND`);
}
