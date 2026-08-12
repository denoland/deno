// A `--preload` module is a strict graph root: unlike `--include` assets, its
// unresolvable imports must still fail compilation. Regression guard for the
// strict-vs-best-effort split introduced for #27505.
import {} from "./does-not-exist.ts";
