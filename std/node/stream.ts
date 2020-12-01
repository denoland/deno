// Copyright Node.js contributors. All rights reserved.

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
import Duplex from "./_stream/duplex.ts";
import eos from "./_stream/end_of_stream.ts";
import PassThrough from "./_stream/passthrough.ts";
import pipeline from "./_stream/pipeline.ts";
import * as promises from "./_stream/promises.ts";
import Readable from "./_stream/readable.ts";
import Stream from "./_stream/stream.ts";
import Transform from "./_stream/transform.ts";
import Writable from "./_stream/writable.ts";

const exports = {
  Duplex,
  finished: eos,
  PassThrough,
  pipeline,
  promises,
  Readable,
  Stream,
  Transform,
  Writable,
};

export default exports;
export {
  Duplex,
  eos as finished,
  PassThrough,
  pipeline,
  promises,
  Readable,
  Stream,
  Transform,
  Writable,
};
