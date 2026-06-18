// Removes a cached npm packument (registry metadata), simulating a precomputed
// cache that has package tarballs but not their registry metadata.
Deno.removeSync(Deno.args[0].trim());
