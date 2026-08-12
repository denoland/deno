// Copyright 2018-2026 the Deno authors. MIT license.
export const kv = await Deno.openKv(":memory:");
kv.listenQueue(async () => {});
