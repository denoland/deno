// Regression test for https://github.com/denoland/deno/issues/29075
//
// AbortSignal.any() with only an AbortController as the source — no timer
// source to keep the event loop alive — must still keep the dependent
// signal alive while it has an abort listener registered. Per the spec
// (https://dom.spec.whatwg.org/#abort-signal-garbage-collection), the
// dependent signal must not be GC'd while its source signals set is
// non-empty and it has registered listeners for the abort event.
//
// Before the fix, the dependent signal returned from AbortSignal.any()
// was only held via WeakRef inside ac.signal's dependentSignals set, so
// V8's GC would collect it after the temporary result of any() went out
// of scope, silently dropping the abort listener.

declare const gc: (opts?: object) => void;

const ac = new AbortController();

AbortSignal.any([ac.signal]).addEventListener("abort", () => {
  console.log("Aborted");
});

await new Promise((resolve) => setTimeout(resolve, 100));

gc({ type: "major", execution: "sync" });

ac.abort();
