const asyncIterable = (async function* () {
  yield* [1, 2, 3];
})();
console.log(ReadableStream.from(asyncIterable));
