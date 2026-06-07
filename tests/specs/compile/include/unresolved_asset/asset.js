// A JS asset with an unresolvable bare import. When passed to `deno compile`
// via `--include`, it must be embedded as an asset rather than analyzed as a
// module graph root (which would fail with a resolution error). See #27505.
import {} from "unknown";

export const x = 1;
