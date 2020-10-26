// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

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
    assert(q.push(1, oneByte));
  }
  assert(!q.push(1, oneByte));
  const [opId, r] = q.shift();
  assert(opId == 1);
  assert(r.byteLength == 1);
  assert(r[0] == 42);
  // Even if we shift one off, we still cannot push a new record.
  assert(!q.push(1, oneByte));
}

function main() {
  const q = Deno.core.sharedQueue;

  const h = q.head();
  assert(h > 0);

  // This record's len is not divisible by
  // 4 so after pushing it to the queue,
  // next record offset should be aligned to 4.
  let r = new Uint8Array([1, 2, 3, 4, 5]);
  const len = r.byteLength + h;
  assert(q.push(1, r));
  // Record should be aligned to 4 bytes
  assert(q.head() == len + 3);

  r = new Uint8Array([6, 7]);
  assert(q.push(1, r));

  r = new Uint8Array([8, 9, 10, 11]);
  assert(q.push(1, r));
  assert(q.numRecords() == 3);
  assert(q.size() == 3);

  let opId;
  [opId, r] = q.shift();
  assert(r.byteLength == 5);
  assert(r[0] == 1);
  assert(r[1] == 2);
  assert(r[2] == 3);
  assert(r[3] == 4);
  assert(r[4] == 5);
  assert(q.numRecords() == 3);
  assert(q.size() == 2);

  [opId, r] = q.shift();
  assert(r.byteLength == 2);
  assert(r[0] == 6);
  assert(r[1] == 7);
  assert(q.numRecords() == 3);
  assert(q.size() == 1);

  [opId, r] = q.shift();
  assert(opId == 1);
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
