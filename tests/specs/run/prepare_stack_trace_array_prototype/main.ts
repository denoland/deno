enum Marker {
  Value,
}

function capture() {
  return new Error(String(Marker.Value)).stack;
}

const originalPrepareStackTrace = Error.prepareStackTrace;
Error.prepareStackTrace = (_, frames) => frames;
let accesses = 0;
function fail() {
  accesses++;
  throw new Error("array prototype accessed");
}

try {
  for (let index = 0; index < 3; index++) {
    Object.defineProperty(Array.prototype, index, {
      get: fail,
      set: fail,
      configurable: true,
    });
    try {
      for (const toStringFirst of [false, true]) {
        const frame = capture()[0];
        const expected = `capture (${import.meta.filename}:6:10)`;
        if (toStringFirst && frame.toString() !== expected) {
          throw new Error("incorrect mapped frame");
        }
        for (let repeat = 0; repeat < 2; repeat++) {
          if (
            frame.getFileName() !== import.meta.filename ||
            frame.getLineNumber() !== 6 ||
            frame.getColumnNumber() !== 10 ||
            frame.toString() !== expected
          ) {
            throw new Error("incorrect mapped location");
          }
        }
      }
    } finally {
      delete Array.prototype[index];
    }
  }
} finally {
  Error.prepareStackTrace = originalPrepareStackTrace;
}
if (accesses !== 0) throw new Error("array prototype accessed");
console.log("ok");
