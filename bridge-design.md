# Proposal for the js-rust bridge.

## High-level overview.

Rather than calling directly into rust through a c++ binding, we run both V8
("the VM") and Rust code ("the backend") in separate threads. Both run run in
the same address space; communication between the two is done through a
wait-free (ring) buffer, and by sharing the memory allocations underlying
ArrayBuffers.

There will be no direct calls from javascript to rust code, and little binding
code in general, although we do need a handful of for purposes of managing
buffers, and to allow the VM thread to go to sleep when it has no events to
process for a prolonged period of time.

## Inter-thread communication

The VM thread sends messages to the backend thread by means of a
SharedArrayBuffer to which both threads have access. Messages are written
linearly into this buffer (each prefixed by the message length). The backend
thread sends the messages back the same way.

The very last int32_t slot of the buffer (let's call it `writer_pos`) is used
for synchronization. Usually it indicates how many bytes have been written by
the producer at the beginning of the buffer, which is also the offset at which
the sender will write the next message.

The consumer spins, polling this field to detect messages appearing in the
buffer. If no new messages appear after spinning for some time, the consumer
thread goes to sleep, and has to be awoken by the producer thread by some other
mechanism (e.g. a futex).

When the buffer is (almost) full, the producer allocates a new buffer and places
a special message in the buffer to point the consumer to the next buffer.

When the consumer has processed all messages in the buffer, and the producer has
moved on to the next buffer, the buffer is released (what this means exactly
depends on whether the consumer is the VM thread or the backend thread - see
below).

## Message serialization

The messages placed in the ring buffer may be of variable length, and are
encoded in some binary format. The exact message protocol is out of scope for
this document.

## Queue consistency

Failure to send a message should not leave the message queue in an inconsistent
state, nor should it evoke resource or memory leaks. Some failure modes: *
Exception thrown during serialization. * Receiving end does not have sufficient
memory available to process the message. * Message is not understood by the
receiving end.

## Transferring buffers

It wouldn't be optimal to place all data in the ring buffer, e.g. when sending a
large chunk of data, the overhead of copying it into and out of the command
buffer might be significant. When data is already in a buffer, we may want to
simply reference them, rather than copying their contents. Likewise, when data
is read, ideally it'd be written directly into a buffer that can be passed off
to the user, rather than in the command buffer.

Therefore an inter-thread message needs to be able to reference a buffer. Since
messages are written into a bytearray, this reference must be a number; an
object reference is of no use because it can't be written into an ArrayBuffer.
The backend thread needs to be able to map this reference to a pointer and a
length (without calling V8 APIs).

V8 allows us to set a custom allocator for ArrayBuffers, but it doesn't give
access to the ArrayBuffer itself, so it's of no use. So we'll add this C++
binding that creates or associates an ArrayBuffer with a unique ID, and stores
this ID plus the [start, length] tuple in a table that can be read by the
backend thread.

## Buffer allocation by the backend thread

The backend thread may need to acquire buffer space. For example, it may need a
buffer to receive data from a socket into, or a buffer to hold response messages
going back to the VM thread. It wouldn't be able to create ArrayBuffers, since
it's not running in the same thread as V8.

In theory, it could call malloc to allocate memory, and leave the responsibility
for creating the corresponding ArrayBuffer object to the VM thread. However this
would require some complicated locking code.

Instead, we leave allocating and freeing buffer space to the VM thread. The
backend thread keeps a pool of "free" buffers which is proactively replenished
by the VM thread.

## Buffer access control

Unfortunately there is no concept of read-only or copy-on-write ArrayBuffers in
V8. It can, however, neuter ArrayBuffers (make them inaccessible).

Message processing on the backend side is security critical; the backend thread
has to validate messages before it processes them. It's important that the VM
thread cannot modify commands after the backend thread has accepted them; it'll
probably have to copy them out of the command buffer before doing so.

## Lifecycle considersions

We must also avoid freeing a buffer when it is garbage collected by V8 while it
is still in use by the backend thread. Not doing so would make it possible for
malicious javascript code to make the backend thread access a buffer after
freeing, which is potentially exploitable.

Therefore the backend thread must be able prevent the buffer backing store from
being freed, although it doesn't have to prevent the ArrayBuffer object itself
from being garbage collected. Javascript code should not be able to modify
override this "locked by the backend" state.

A way to do this would be by adding a reference count to the buffer metatadata.
The backend thread can "capture" a buffer by increasing the reference count. If
the JS thread holds a strong reference to the ArrayBuffer object, the reference
count is increased by one (1).

The reference count can be increased/decreased using atomic operations, so the
backend thread can directly modify this value. The VM thread is still
responsible for keeping the buffer alive up to the moment that the backend
thread captures it, so generally it'll have to maintain a strong reference to
buffers that are in use by the backend thread.

```cpp
struct deno_buf {
  Handle<ArrayBuffer> handle;  // May be NULL if allocated but not mapped.
  void* base;                  // Start address.
  uint32_t length;             // Maximum ArrayBuffer size is 2^32 bytes.
  volatile uint32_t ref_count; // Reference count as described above.
}

struct deno_buf buffer_table[MAX_BUFS];
```

## Data buffers

Depending on how pedantic we want to be, we also may want to make data buffers
that are transferred to the backend thread inaccessible to the VM thread,
although it's probably not strictly necessary for security reasons.

## Strings

Since v8 doesn't have built-in methods to convert between strings and buffers,
we have to add something for it. The most obvious existing API would be the
TextEncoder and TextDecoder APIs, although they are kinda limited in that they
can only convert strings (in their entirety) to an Uint8Array backed by a newly
allocated ArrayBuffer.

This has to be implemented as a binding for efficiency reasons (except maybe for
really short strings). We can have an API that both converts a string to a
buffer, and assigns an ID to this buffer in one go.

### Binding API

```ts
deno.bufctl(op: BufOp,
            id: number,
            arg: number | string | ArrayBuffer): ArrayBuffer | null.

enum BufOp {
  ALLOC,  // Allocate a new buffer and assign `id`. `arg` is the length.
  FREE,   // Free buffer with `id`; neuter the ArrayBuffer object, if any.
  ASSIGN, // Assign `id` to an existing ArrayBuffer. `arg` is the buffer.
  MAP,    // Get the ArrayBuffer associated with `id`, or create one.  
  UNMAP   // Neuter the ArrayBuffer associated with `id`, but don't free.
}
```
