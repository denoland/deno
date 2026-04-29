# op2

`#[op2]` is the in-progress replacement for `#[op]`.

## Strings

`String`s in Rust are always UTF-8. `String`s in v8, however, are either
two-byte UTF-16 or one-byte Latin-1. One-byte Latin-1 strings are not
byte-compatible with UTF-8, as characters with the index 128-255 require two
bytes to encode in UTF-8.

Because of this, `String`s in `op`s always require a copy (at least) to ensure
that we are not incorrectly passing Latin-1 data to methods that expect a UTF-8
string. At this time there is no way to avoid this copy, though the `op` code
does attempt to avoid any allocations where possible by making use of a stack
buffer.

## Fallible `op`s

An `op` function may be declared to return `Result` to indicate that the `op` is
fallible. The error type must implement `deno_error::JsErrorClass`. When the
function returns `Err`, an exception is thrown.

## `async` calls

Asynchronous calls are fully inferred from the function definition. Asynchronous
calls are supported in two forms:

```rust,ignore
async fn op_xyz(/* ... */) -> X {}
```

and

```rust,ignore
fn op_xyz(/* ... */) -> impl Future<Output = X> {}
```

These are desugared to a function that adds a hidden `promise_id` argument, and
returns `Option<X>` instead. Deno will eagerly poll the op, and if it is
immediately ready, the function will return `Some(X)`. If the op is not ready,
the function will return `None` and the future will be handled by Deno's pending
op system.

```rust,ignore
fn op_xyz(promise_id: i32 /* ... */) -> Option<X> {}
```

### Eager `async` calls

By default, `async` functions are eagerly polled, which reduces the latency of
the call dramatically if the async function is ready to return a value
immediately.

### `async(lazy)`

`async` calls may be marked as `lazy`, which allows the runtime to defer polling
the op until a later time. The submission of an `async(lazy)` op might be
faster, but the latency will be higher for ops that would have been ready on the
first poll.

**NOTE**: You _may_ need to use this to get the maximum performance out of a set
of async tasks, but it should only be used alongside careful benchmarking. In
some cases it will allow for higher throughput at the expense of latency.

Lazy `async` calls _may_ be fastcalls, though the resolution will still happen
on a slow path.

### `async(deferred)`

`async` calls may also be marked as `deferred`, which will allow the runtime to
poll the op immediately, but any results that are ready are deferred until a
later run of the event loop.

**NOTE**: This is almost certainly not what you want to use and should only be
used if you really know what you are doing.

Lazy `async(deferred)` calls _may_ be fastcalls, though the resolution will
still happen on a slow path.

## fastcalls

`op2` requires fastcall-compatible ops to be annotated with `fast`. If you wish
to avoid fastcalls for some reason (this is unlikely), you can specify `nofast`
instead.

You may also choose an alternate op function to use as the fastcall equivalent
to a slow function. In this case, you can specify `fast(op_XYZ)`. The other op
must be decorated with `#[op2(fast)]`, and does not need to be registered. When
v8 optimized the slow function to a fastcall, it will switch the implementation
over if the parameters are compatible. This is useful for a function that takes
any buffer type in the slow path and wishes to use the very fast typed `u8`
buffer for the fast path.

## Argument conversion

Arguments in non-fast ops use the `deno_core::convert::FromV8Scopeless` trait by
default. This trait does not require a v8 scope for conversion, making it more
efficient for many types.

To use the `FromV8` trait instead (which provides access to a v8 scope during
conversion), add the `#[scoped]` attribute to the argument:

```rust,ignore
fn op_xyz(#[scoped] arg: MyFromV8Type) -> X {}
```

## Return value conversion

Return types use the `ToV8` trait by default in non-fast ops. Any type that
implements `deno_core::convert::ToV8` can be returned directly without any
attribute.

# CppGC Objects

`op2` supports defining native JavaScript classes backed by Rust types using
V8's CppGC (C++ garbage collector). These objects live on the V8 heap and are
automatically garbage collected.

## Basic usage

Define a struct that implements `GarbageCollected`, then use `#[op2]` on an
`impl` block to define its JavaScript API:

```rust,ignore
use deno_core::GarbageCollected;
use deno_core::v8::cppgc::GcCell;

#[repr(C)]
pub struct MyObject {
  value: GcCell<f64>,
}

unsafe impl GarbageCollected for MyObject {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"MyObject"
  }
}

#[op2]
impl MyObject {
  #[constructor]
  #[cppgc]
  fn new(value: f64) -> MyObject {
    MyObject {
      value: GcCell::new(value),
    }
  }

  #[getter]
  fn value(&self, isolate: &v8::Isolate) -> f64 {
    *self.value.get(isolate)
  }

  #[setter]
  fn value(&self, isolate: &mut v8::Isolate, value: f64) {
    self.value.set(isolate, value);
  }

  #[fast]
  fn double_value(&self, isolate: &v8::Isolate) -> f64 {
    *self.value.get(isolate) * 2.0
  }

  #[static_method]
  #[cppgc]
  fn create(value: f64) -> MyObject {
    MyObject {
      value: GcCell::new(value),
    }
  }
}
```

Register the object in your extension:

```rust,ignore
deno_core::extension!(
  my_ext,
  objects = [MyObject],
  // ...
);
```

The object is then available in JavaScript:

```js
import { MyObject } from "ext:core/ops";

const obj = new MyObject(42);
console.log(obj.value); // 42
console.log(obj.doubleValue()); // 84
obj.value = 10;
```

### Supported member types

- `#[constructor]` — The JS constructor. Must return the struct type (optionally
  wrapped in `Result`). Mark with `#[cppgc]` to indicate the return is a CppGC
  object.
- `#[getter]` / `#[setter]` — Property accessors. Getter and setter for the same
  property should share the same function name.
- `#[static_method]` — A static method on the class (e.g., `MyObject.create()`).
- `#[fast]` — Regular methods. Use `&self` as the first parameter to receive the
  native object.

## Inheritance

CppGC objects support a prototype-based inheritance model that mirrors
JavaScript's class inheritance. This allows you to define a base class in Rust
and have derived classes inherit its methods and properties, just like
`class Child extends Parent` in JavaScript.

### Defining a base class

Mark the struct with `#[derive(CppgcBase)]` and its `impl` block with
`#[op2(base)]`:

```rust,ignore
use deno_core::CppgcBase;

#[derive(CppgcBase)]
#[repr(C)]
pub struct Shape {
  sides: GcCell<u32>,
}

unsafe impl GarbageCollected for Shape {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Shape"
  }
}

#[op2(base)]
impl Shape {
  #[constructor]
  #[cppgc]
  fn new(sides: u32) -> Shape {
    Shape {
      sides: GcCell::new(sides),
    }
  }

  #[getter]
  fn sides(&self, isolate: &v8::Isolate) -> u32 {
    *self.sides.get(isolate)
  }
}
```

The `base` attribute tells `op2` to use a polymorphic unwrap when accessing
`&self`, so that methods on `Shape` can be called on any type that inherits from
it.

### Defining a derived class

Mark the struct with `#[derive(CppgcInherits)]`, put the base type as the first
field, and use `#[op2(inherit = BaseType)]` on the `impl` block:

```rust,ignore
use deno_core::CppgcInherits;

#[derive(CppgcInherits)]
#[cppgc_inherits_from(Shape)]
#[repr(C)]
pub struct Rectangle {
  base: Shape, // must be the first field
  width: GcCell<f64>,
  height: GcCell<f64>,
}

unsafe impl GarbageCollected for Rectangle {
  fn trace(&self, _visitor: &mut v8::cppgc::Visitor) {}
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Rectangle"
  }
}

#[op2(inherit = Shape)]
impl Rectangle {
  #[constructor]
  #[cppgc]
  fn new(width: f64, height: f64) -> Rectangle {
    Rectangle {
      base: Shape {
        sides: GcCell::new(4),
      },
      width: GcCell::new(width),
      height: GcCell::new(height),
    }
  }

  #[fast]
  fn area(&self, isolate: &v8::Isolate) -> f64 {
    *self.width.get(isolate) * *self.height.get(isolate)
  }
}
```

In JavaScript, `Rectangle` inherits from `Shape`:

```js
const rect = new Rectangle(3, 4);
console.log(rect.sides); // 4 (inherited from Shape)
console.log(rect.area()); // 12
console.log(rect instanceof Rectangle); // true
console.log(rect instanceof Shape); // true
```

JavaScript classes can also extend these native classes:

```js
class Square extends Rectangle {
  constructor(size) {
    super(size, size);
  }
}
const sq = new Square(5);
console.log(sq.area()); // 25
console.log(sq.sides); // 4
```

### Multi-level inheritance

If a derived class will itself be inherited from, it must also be a base class.
Derive both `CppgcInherits` and `CppgcBase`, and use
`#[op2(base, inherit = ParentType)]`:

```rust,ignore
// Rectangle is both a child of Shape AND a base for further derivation.
#[derive(CppgcInherits, CppgcBase)]
#[cppgc_inherits_from(Shape)]
#[repr(C)]
pub struct Rectangle {
  base: Shape,
  width: GcCell<f64>,
  height: GcCell<f64>,
}

#[op2(base, inherit = Shape)]
impl Rectangle {
  // ... methods ...
}

// Square inherits from Rectangle
#[derive(CppgcInherits)]
#[cppgc_inherits_from(Rectangle)]
#[repr(C)]
pub struct Square {
  base: Rectangle,
}

#[op2(inherit = Rectangle)]
impl Square {
  // ... methods ...
}
```

### Requirements

- All types in the inheritance chain must use `#[repr(C)]`.
- The base type must be the **first field** of the derived struct, and it must
  be at offset 0.
- Leaf types (not inherited from) only need `#[derive(CppgcInherits)]`.
- Root base types only need `#[derive(CppgcBase)]`.
- Types in the middle of the chain need both
  `#[derive(CppgcInherits, CppgcBase)]`.
- All types must be registered in the extension's `objects = [...]` list, with
  base types listed **before** their derived types.

# Parameters

<!-- START ARGS -->
<table><tr><th>Rust</th><th>Fastcall</th><th>v8</th></tr>
<tr>
<td>

```text
bool
```

</td><td>
✅
</td><td>
Bool
</td><td>

</td></tr>
<tr>
<td>

```text
i8
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
u8
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
i16
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
u16
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
i32
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
u32
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
#[smi] ResourceId
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>
SMI is internally represented as a signed integer, but unsigned `#[smi]` types will be bit-converted to unsigned values for the Rust call. JavaScript code will continue to see signed integers.
</td></tr>
<tr>
<td>

```text
#[bigint] i64
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] u64
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] isize
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] usize
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
f32
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
f64
```

</td><td>
✅
</td><td>
Uint32, Int32, Number, BigInt
</td><td>

</td></tr>
<tr>
<td>

```text
#[string] String
```

</td><td>
✅
</td><td>
String
</td><td>
Fastcall available only if string is Latin-1. Will always create an allocated, UTF-8 copy of the String data.
</td></tr>
<tr>
<td>

```text
#[string] &str
```

</td><td>
✅
</td><td>
String
</td><td>
Fastcall available only if string is Latin-1. Will create an owned `String` copy of the String data if it doesn't fit on the stack. Will never allocate in a fastcall, but will copy Latin-1 -> UTF-8.
</td></tr>
<tr>
<td>

```text
#[string] Cow<str>
```

</td><td>
✅
</td><td>
String
</td><td>
Fastcall available only if string is Latin-1. Will create a `Cow::Owned` copy of the String data if it doesn't fit on the stack. Will always be `Cow::Borrowed` in a fastcall, but will copy Latin-1 -> UTF-8.
</td></tr>
<tr>
<td>

```text
#[string(onebyte)] Cow<[u8]>
```

</td><td>
✅
</td><td>
String
</td><td>
Fastest `String`-type method. If the string is not Latin-1, will throw a TypeError.
</td></tr>
<tr>
<td>

```text
&v8::Value
```

</td><td>
✅
</td><td>
any
</td><td>

</td></tr>
<tr>
<td>

```text
&v8::String
```

</td><td>
✅
</td><td>
String
</td><td>

</td></tr>
<tr>
<td>

```text
&v8::Object
```

</td><td>
✅
</td><td>
Object
</td><td>

</td></tr>
<tr>
<td>

```text
&v8::Function
```

</td><td>
✅
</td><td>
Function
</td><td>

</td></tr>
<tr>
<td>

```text
&v8::...
```

</td><td>
✅
</td><td>
...
</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::Value>
```

</td><td>
✅
</td><td>
any
</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::String>
```

</td><td>
✅
</td><td>
String
</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::Object>
```

</td><td>
✅
</td><td>
Object
</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::Function>
```

</td><td>
✅
</td><td>
Function
</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::...>
```

</td><td>
✅
</td><td>
...
</td><td>

</td></tr>
<tr>
<td>

```text
FromV8Scopeless
```

</td><td>

</td><td>
any
</td><td>
Any type that implements `deno_core::covert::FromV8Scopeless`.
</td></tr>
<tr>
<td>

```text
#[scoped] FromV8Type
```

</td><td>

</td><td>
any
</td><td>
Any type that implements `deno_core::covert::FromV8`. ⚠️ May be slow.
</td></tr>
<tr>
<td>

```text
#[scoped] (Tuple, Tuple)
```

</td><td>

</td><td>
any
</td><td>
Any type that implements `deno_core::covert::FromV8`. ⚠️ May be slow.
</td></tr>
<tr>
<td>

```text
#[serde] SerdeType
```

</td><td>

</td><td>
any
</td><td>
⚠️ May be slow. Legacy & not recommended, use `FromV8` trait and macros instead.
</td></tr>
<tr>
<td>

```text
#[serde] (Tuple, Tuple)
```

</td><td>

</td><td>
any
</td><td>
⚠️ May be slow. Legacy & not recommended, use `FromV8` trait and macros instead.
</td></tr>
<tr>
<td>

```text
#[arraybuffer] &mut [u8]
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
⚠️ JS may modify the contents of the slice if V8 is called re-entrantly.
</td></tr>
<tr>
<td>

```text
#[arraybuffer] &[u8]
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
⚠️ JS may modify the contents of the slice if V8 is called re-entrantly.
</td></tr>
<tr>
<td>

```text
#[arraybuffer] *mut u8
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
⚠️ JS may modify the contents of the slice if V8 is called re-entrantly. Because of how V8 treats empty arrays in fastcalls, they will always be passed as null.
</td></tr>
<tr>
<td>

```text
#[arraybuffer] *const u8
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
⚠️ JS may modify the contents of the slice if V8 is called re-entrantly. Because of how V8 treats empty arrays in fastcalls, they will always be passed as null.
</td></tr>
<tr>
<td>

```text
#[arraybuffer(copy)] Vec<u8>
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[arraybuffer(copy)] Box<[u8]>
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[arraybuffer(copy)] bytes::Bytes
```

</td><td>
✅
</td><td>
ArrayBuffer (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[buffer(copy)] Vec<u8>
```

</td><td>
✅
</td><td>
UInt8Array (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[buffer(copy)] Box<[u8]>
```

</td><td>
✅
</td><td>
UInt8Array (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[buffer(copy)] bytes::Bytes
```

</td><td>
✅
</td><td>
UInt8Array (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[buffer] &mut [u32]
```

</td><td>
✅
</td><td>
UInt32Array (resizable=true,false)
</td><td>
⚠️ JS may modify the contents of the slice if V8 is called re-entrantly.
</td></tr>
<tr>
<td>

```text
#[buffer] &[u32]
```

</td><td>
✅
</td><td>
UInt32Array (resizable=true,false)
</td><td>
⚠️ JS may modify the contents of the slice if V8 is called re-entrantly.
</td></tr>
<tr>
<td>

```text
#[buffer(copy)] Vec<u32>
```

</td><td>
✅
</td><td>
UInt32Array (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[buffer(copy)] Box<[u32]>
```

</td><td>
✅
</td><td>
UInt32Array (resizable=true,false)
</td><td>
Safe, but forces a copy.
</td></tr>
<tr>
<td>

```text
#[buffer(detach)] JsBuffer
```

</td><td>

</td><td>
ArrayBufferView (resizable=true,false)
</td><td>
Safe.
</td></tr>
<tr>
<td>

```text
*const std::ffi::c_void
```

</td><td>
✅
</td><td>
External
</td><td>

</td></tr>
<tr>
<td>

```text
*mut std::ffi::c_void
```

</td><td>
✅
</td><td>
External
</td><td>

</td></tr>
<tr>
<td>

```text
&OpState
```

</td><td>
✅
</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
&mut OpState
```

</td><td>
✅
</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
Rc<RefCell<OpState>>
```

</td><td>
✅
</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
&JsRuntimeState
```

</td><td>
✅
</td><td>

</td><td>
Only usable in `deno_core`.
</td></tr>
</table>

<!-- END ARGS -->

# Return Values

<!-- START RV -->
<table><tr><th>Rust</th><th>Fastcall</th><th>Async</th><th>v8</th></tr>
<tr>
<td>

```text
bool
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
i8
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
u8
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
i16
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
u16
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
i32
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
u32
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[smi] ResourceId
```

</td><td>
✅
</td><td>

</td><td>
SMI is internally represented as a signed integer, but unsigned `#[smi]` types will be bit-converted to unsigned values for the Rust call. JavaScript code will continue to see signed integers.
</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] i64
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] u64
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] isize
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[bigint] usize
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[number] i64
```

</td><td>
✅
</td><td>

</td><td>
Result must fit within `Number.MIN_SAFE_INTEGER` and `Number.MAX_SAFE_INTEGER`
</td><td>

</td></tr>
<tr>
<td>

```text
#[number] u64
```

</td><td>
✅
</td><td>

</td><td>
Result must fit within `Number.MIN_SAFE_INTEGER` and `Number.MAX_SAFE_INTEGER`
</td><td>

</td></tr>
<tr>
<td>

```text
#[number] isize
```

</td><td>
✅
</td><td>

</td><td>
Result must fit within `Number.MIN_SAFE_INTEGER` and `Number.MAX_SAFE_INTEGER`
</td><td>

</td></tr>
<tr>
<td>

```text
#[number] usize
```

</td><td>
✅
</td><td>

</td><td>
Result must fit within `Number.MIN_SAFE_INTEGER` and `Number.MAX_SAFE_INTEGER`
</td><td>

</td></tr>
<tr>
<td>

```text
f32
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
f64
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[string] String
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[string] &str
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[string] Cow<str>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[string(onebyte)] Cow<[u8]>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[arraybuffer] V8Slice<u8>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[arraybuffer] Vec<u8>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[arraybuffer] Box<[u8]>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[arraybuffer] bytes::BytesMut
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[buffer] V8Slice<u8>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[buffer] Vec<u8>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[buffer] Box<[u8]>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[buffer] bytes::BytesMut
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
#[buffer] V8Slice<u32>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
*const std::ffi::c_void
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
*mut std::ffi::c_void
```

</td><td>
✅
</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::Value>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::String>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::Object>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::Function>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
v8::Local<v8::...>
```

</td><td>

</td><td>

</td><td>

</td><td>

</td></tr>
<tr>
<td>

```text
ToV8Type
```

</td><td>

</td><td>

</td><td>
Any type that implements `deno_core::covert::ToV8`.
</td><td>

</td></tr>
<tr>
<td>

```text
(ToV8Type, ToV8Type)
```

</td><td>

</td><td>

</td><td>
Any type that implements `deno_core::covert::ToV8`.
</td><td>

</td></tr>
<tr>
<td>

```text
#[serde] SerdeType
```

</td><td>

</td><td>

</td><td>
⚠️ Legacy & not recommended, use `ToV8` trait and macros instead.
</td><td>

</td></tr>
<tr>
<td>

```text
#[serde] (SerdeType, SerdeType)
```

</td><td>

</td><td>

</td><td>
⚠️ Legacy & not recommended, use `ToV8` trait and macros instead.
</td><td>

</td></tr>
</table>

<!-- END RV -->
