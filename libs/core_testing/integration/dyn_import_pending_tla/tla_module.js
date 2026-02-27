// Copyright 2018-2026 the Deno authors. MIT license.

// Module with Top-Level Await
await new Promise((resolve) => setTimeout(resolve, 100));
export default "tla module loaded";
