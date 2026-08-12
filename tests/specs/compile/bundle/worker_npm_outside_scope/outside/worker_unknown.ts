// Negative case: this worker imports a package that is not in the build's npm
// snapshot at all. The snapshot fallback must not silently swallow it; the
// build is expected to fail rather than produce a binary with an unresolved
// import.
import "@denotest/this-package-does-not-exist";

self.onmessage = () => {
  (self as unknown as Worker).postMessage("unreachable");
};
