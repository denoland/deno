if ("Deno" in globalThis && typeof globalThis.Deno === 'object') {
  console.log('deno preinstall.js');
} else {
  console.log('node preinstall.js');
}