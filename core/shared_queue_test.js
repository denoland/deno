// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

function assert(cond) {
  if (!cond) {
    throw Error("assert");
  }
}

// Check overflow (corresponds to full_records test in rust)
function fullRecords(q) {
  q.reset();
  const oneByte = new Uint8Array([42]);
  for (let i = 0; i < q.MAX_RECORDS; i++) {
    assert(q.push(oneByte));
  }
  assert(!q.push(oneByte));
  r = q.shift();
  assert(r.byteLength == 1);
  assert(r[0] == 42);
  // Even if we shift one off, we still cannot push a new record.
  assert(!q.push(oneByte));
}

function main() {
  const q = Deno.core.sharedQueue;

  let h = q.head();
  assert(h > 0);

  let r = new Uint8Array([1, 2, 3, 4, 5]);
  let len = r.byteLength + h;
  assert(q.push(r));
  assert(q.head() == len);

  r = new Uint8Array([6, 7]);
  assert(q.push(r));

  r = new Uint8Array([8, 9, 10, 11]);
  assert(q.push(r));
  assert(q.numRecords() == 3);
  assert(q.size() == 3);

  r = q.shift();
  assert(r.byteLength == 5);
  assert(r[0] == 1);
  assert(r[1] == 2);
  assert(r[2] == 3);
  assert(r[3] == 4);
  assert(r[4] == 5);
  assert(q.numRecords() == 3);
  assert(q.size() == 2);

  r = q.shift();
  assert(r.byteLength == 2);
  assert(r[0] == 6);
  assert(r[1] == 7);
  assert(q.numRecords() == 3);
  assert(q.size() == 1);

  r = q.shift();
  assert(r.byteLength == 4);
  assert(r[0] == 8);
  assert(r[1] == 9);
  assert(r[2] == 10);
  assert(r[3] == 11);
  assert(q.numRecords() == 0);
  assert(q.size() == 0);

  assert(q.shift() == null);
  assert(q.shift() == null);
  assert(q.numRecords() == 0);
  assert(q.size() == 0);

  fullRecords(q);

  Deno.core.print("shared_queue_test.js ok\n");
  q.reset();
}

main();
