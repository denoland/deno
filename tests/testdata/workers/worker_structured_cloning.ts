// More info on structured cloning can be found here:
// https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Structured_clone_algorithm

self.onmessage = () => {
  const arr = ["a", true, 432];
  const set = new Set([1, 3, 5, 7, 9]);
  const selfReference = {
    a: arr,
    b: arr,
    c: set,
  };
  // deno-lint-ignore no-explicit-any
  (selfReference as any).self = selfReference;
  self.postMessage(selfReference);
};
