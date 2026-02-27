// Copyright 2018-2025 the Deno authors. MIT license.
import {
  assert,
  assertArrayEquals,
  assertEquals,
  assertThrows,
  test,
} from "checkin:testing";
import {
  DOMPoint,
  DOMPointReadOnly,
  TestEnumWrap,
  TestObjectWrap,
} from "checkin:object";

const {
  op_pipe_create,
  op_file_open,
  op_async_make_cppgc_resource,
  op_async_get_cppgc_resource,
} = Deno.core.ops;

test(async function testPipe() {
  const [p1, p2] = op_pipe_create();
  assertEquals(3, await Deno.core.write(p1, new Uint8Array([1, 2, 3])));
  const buf = new Uint8Array(10);
  assertEquals(3, await Deno.core.read(p2, buf));
  assertArrayEquals(buf.subarray(0, 3), [1, 2, 3]);
});

test(async function testPipeSmallRead() {
  const [p1, p2] = op_pipe_create();
  assertEquals(
    6,
    await Deno.core.write(p1, new Uint8Array([1, 2, 3, 4, 5, 6])),
  );
  const buf = new Uint8Array(1);
  for (let i = 1; i <= 6; i++) {
    assertEquals(1, await Deno.core.read(p2, buf));
    assertArrayEquals(buf.subarray(0), [i]);
  }
});

test(async function opsAsyncBadResource() {
  try {
    const nonExistingRid = 9999;
    await Deno.core.read(
      nonExistingRid,
      new Uint8Array(100),
    );
  } catch (e) {
    assert(e instanceof Deno.core.BadResource);
  }
});

test(function opsSyncBadResource() {
  try {
    const nonExistingRid = 9999;
    Deno.core.readSync(
      nonExistingRid,
      new Uint8Array(100),
    );
  } catch (e) {
    assert(e instanceof Deno.core.BadResource);
  }
});

test(async function testFileIsNotTerminal() {
  const file = await op_file_open("./README.md", true);
  assert(!Deno.core.isTerminal(file));
});

test(async function testFileReadUnref() {
  const file = await op_file_open("./README.md", true);

  let called = false;
  await Deno.core.read(file, new Uint8Array(100))
    .then(() => {
      called = true;
    });
  assert(called);

  const file2 = await op_file_open("./README.md", false);
  Deno.core.read(file2, new Uint8Array(100))
    .then(() => {
      throw new Error("should not be called");
    });
});

test(async function testCppgcAsync() {
  const resource = await op_async_make_cppgc_resource();
  assertEquals(await op_async_get_cppgc_resource(resource), 42);
});

test(async function testDomPoint() {
  const p1 = new DOMPoint(100, 100);
  const p2 = new DOMPoint();
  const p3 = DOMPoint.fromPoint({ x: 200 });
  const p4 = DOMPoint.fromPoint({ x: 0, y: 100, z: 99.9, w: 100 });
  const p5 = p1.fromPoint({ x: 200 });
  assertEquals(p1.x, 100);
  assertEquals(p2.x, 0);
  assertEquals(p3.x, 200);
  assertEquals(p4.x, 0);
  assertEquals(p5.x, 200);

  assertEquals(DOMPoint.fromPoint.length, 1);
  const { get, set } = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(p1),
    "x",
  );
  assertEquals(get.name, "get x");
  assertEquals(get.length, 0);
  assertEquals(set.name, "set x");
  assertEquals(set.length, 1);

  assert(p1 instanceof DOMPoint);
  assert(p1 instanceof DOMPointReadOnly);

  assertEquals("prototype" in DOMPoint.prototype.wrappingSmi, false);

  let caught;
  try {
    // @ts-expect-error bad arg test
    new DOMPoint("bad");
  } catch (e) {
    caught = e;
  }
  assert(caught);

  const u32Max = 4294967295; // test wrapping for smi
  assertEquals(p1.wrappingSmi(u32Max), u32Max);
  assertEquals(p1.wrappingSmi(u32Max + 1), 0);
  assertEquals(p1.wrappingSmi(u32Max + 2), 1);

  assertEquals(
    p1.wrappingSmi.toString(),
    DOMPoint.prototype.wrappingSmi.toString(),
  );

  const f = Symbol.for("symbolMethod");
  p1[f]();

  const wrap = new TestObjectWrap();
  assertEquals(wrap.withVarargs(1, 2, 3), 3);
  assertEquals(wrap.withVarargs(1, 2, 3, 4, 5), 5);
  assertEquals(wrap.withVarargs(), 0);
  assertEquals(wrap.withVarargs(undefined), 1);

  wrap.withThis();
  wrap.with_RENAME();

  assert(wrap.undefinedResult() === undefined);
  assert(wrap.undefinedUnit() === undefined);

  wrap.withValidateInt(10);

  assertThrows(
    () => {
      // @ts-expect-error bad arg test
      wrap.withValidateInt(2, 2);
    },
    TypeError,
    "Expected one argument",
  );

  assertThrows(
    () => {
      // @ts-expect-error bad arg test
      wrap.withValidateInt("bad");
    },
    TypeError,
    "Expected int",
  );

  const promise = wrap.withAsyncFn(10);
  assert(promise instanceof Promise);

  await promise;

  new TestEnumWrap();
});

// TODO(littledivy): write this test using natives api when exposed
test(function testFastProtoMethod() {
  const obj = new TestObjectWrap();

  for (let i = 0; i < 10000; i++) {
    obj.withScopeFast(); // trigger fast call
  }
});
