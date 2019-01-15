// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#ifndef DENO_H_
#define DENO_H_
#include <stddef.h>
#include <stdint.h>
// Neither Rust nor Go support calling directly into C++ functions, therefore
// the public interface to libdeno is done in C.
#ifdef __cplusplus
extern "C" {
#endif

// Data that gets transmitted.
typedef struct {
  uint8_t* alloc_ptr;  // Start of memory allocation (returned from `malloc()`).
  size_t alloc_len;    // Length of the memory allocation.
  uint8_t* data_ptr;   // Start of logical contents (within the allocation).
  size_t data_len;     // Length of logical contents.
} deno_buf;

typedef struct deno_s Deno;

// A callback to receive a message from a libdeno.send() javascript call.
// control_buf is valid for only for the lifetime of this callback.
// data_buf is valid until deno_respond() is called.
typedef void (*deno_recv_cb)(void* user_data, int32_t req_id,
                             deno_buf control_buf, deno_buf data_buf);

// A callback to implement ES Module imports. User must call deno_resolve_ok()
// at most once during deno_resolve_cb. If deno_resolve_ok() is not called, the
// specifier is considered invalid and will issue an error in JS. The reason
// deno_resolve_cb does not return deno_module is to avoid unnecessary heap
// allocations.
typedef void (*deno_resolve_cb)(void* user_data, const char* specifier,
                                const char* referrer);

void deno_resolve_ok(Deno* d, const char* filename, const char* source);

void deno_init();
const char* deno_v8_version();
void deno_set_v8_flags(int* argc, char** argv);

typedef struct {
  int will_snapshot;           // Default 0. If calling deno_get_snapshot 1.
  deno_buf load_snapshot;      // Optionally: A deno_buf from deno_get_snapshot.
  deno_buf shared;             // Shared buffer to be mapped to libdeno.shared
  deno_recv_cb recv_cb;        // Maps to libdeno.send() calls.
  deno_resolve_cb resolve_cb;  // Each import calls this.
} deno_config;

// Create a new deno isolate.
// Warning: If config.will_snapshot is set, deno_get_snapshot() must be called
// or an error will result.
Deno* deno_new(deno_config config);

// Generate a snapshot. The resulting buf can be used with deno_new.
// The caller must free the returned data by calling delete[] buf.data_ptr.
deno_buf deno_get_snapshot(Deno* d);

void deno_delete(Deno* d);

// Compile and execute a traditional JavaScript script that does not use
// module import statements.
// Return value: 0 = fail, 1 = success
// Get error text with deno_last_exception().
//
// TODO change return value to be const char*. On success the return
// value is nullptr, on failure it is the JSON exception text that
// is returned by deno_last_exception(). Remove deno_last_exception().
// The return string is valid until the next execution of deno_execute or
// deno_respond (as deno_last_exception is now).
int deno_execute(Deno* d, void* user_data, const char* js_filename,
                 const char* js_source);

// Compile and execute an ES module. Caller must have provided a deno_resolve_cb
// when instantiating the Deno object.
// Return value: 0 = fail, 1 = success
// Get error text with deno_last_exception().
// If resolve_only is 0, compile and evaluate the module.
// If resolve_only is 1, compile and collect dependencies of the module
// without running the code.
int deno_execute_mod(Deno* d, void* user_data, const char* js_filename,
                     const char* js_source, int resolve_only);

// deno_respond sends up to one message back for every deno_recv_cb made.
//
// If this is called during deno_recv_cb, the issuing libdeno.send() in
// javascript will synchronously return the specified buf as an ArrayBuffer (or
// null if buf is empty).
//
// If this is called after deno_recv_cb has returned, the deno_respond
// will call into the JS callback specified by libdeno.recv().
//
// (Ideally, but not currently: After calling deno_respond(), the caller no
// longer owns `buf` and must not use it; deno_respond() is responsible for
// releasing its memory.)
//
// Calling this function more than once with the same req_id will result in
// an error.
//
// A non-zero return value, means a JS exception was encountered during the
// libdeno.recv() callback. Check deno_last_exception() for exception text.
//
// TODO change return value to be const char*. On success the return
// value is nullptr, on failure it is the JSON exception text that
// is returned by deno_last_exception(). Remove deno_last_exception().
// The return string is valid until the next execution of deno_execute or
// deno_respond (as deno_last_exception is now).
int deno_respond(Deno* d, void* user_data, int32_t req_id, deno_buf buf);

void deno_check_promise_errors(Deno* d);

const char* deno_last_exception(Deno* d);

void deno_terminate_execution(Deno* d);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
