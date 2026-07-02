// Probe Deno.openKv() under DENO_KV_REQUIRES_DISTRIBUTED_DATABASE.
console.log("typeof openKv:", typeof Deno.openKv);
try {
  const kv = await Deno.openKv();
  console.log("opened ok");
  kv.close();
} catch (e) {
  console.log("caught:", (e as Error).message);
}
