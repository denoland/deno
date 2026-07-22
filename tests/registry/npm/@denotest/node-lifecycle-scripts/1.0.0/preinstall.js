if ("Deno" in globalThis && typeof globalThis.Deno === 'object') {
  require('./helper.js');
  console.log('deno preinstall.js');
} else {
  console.log('node preinstall.js');
}