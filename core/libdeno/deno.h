// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
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
  size_t zero_copy_id;  // 0 = normal, 1 = must call deno_zero_copy_release.
} deno_buf;

typedef struct {
  uint8_t* data_ptr;
  size_t data_len;
} deno_snapshot;

typedef struct deno_s Deno;

// A callback to receive a message from a libdeno.send() javascript call.
// control_buf is valid for only for the lifetime of this callback.
// data_buf is valid until deno_respond() is called.
typedef void (*deno_recv_cb)(void* user_data, deno_buf control_buf,
                             deno_buf zerop_copy_buf);

void deno_init();
const char* deno_v8_version();
void deno_set_v8_flags(int* argc, char** argv);

typedef struct {
  int will_snapshot;            // Default 0. If calling deno_get_snapshot 1.
  deno_snapshot load_snapshot;  // A startup snapshot to use.
  deno_buf shared;              // Shared buffer to be mapped to libdeno.shared
  deno_recv_cb recv_cb;         // Maps to libdeno.send() calls.
} deno_config;

// Create a new deno isolate.
// Warning: If config.will_snapshot is set, deno_get_snapshot() must be called
// or an error will result.
Deno* deno_new(deno_config config);
void deno_delete(Deno* d);

// Generate a snapshot. The resulting buf can be used in as the load_snapshot
// member in deno_confg.
// When calling this function, the caller must have created the isolate "d" with
// "will_snapshot" set to 1.
// The caller must free the returned data with deno_snapshot_delete().
deno_snapshot deno_get_snapshot(Deno* d);

// Only for use with data returned from deno_get_snapshot.
void deno_snapshot_delete(deno_snapshot);

void deno_lock(Deno* d);
void deno_unlock(Deno* d);

// Compile and execute a traditional JavaScript script that does not use
// module import statements.
// If it succeeded deno_last_exception() will return NULL.
void deno_execute(Deno* d, void* user_data, const char* js_filename,
                  const char* js_source);

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
// If a JS exception was encountered, deno_last_exception() will be non-NULL.
void deno_respond(Deno* d, void* user_data, deno_buf buf);

// consumes zero_copy
// Calling this function more than once with the same zero_copy_id will result
// in an error.
void deno_zero_copy_release(Deno* d, size_t zero_copy_id);

void deno_check_promise_errors(Deno* d);

const char* deno_last_exception(Deno* d);

void deno_terminate_execution(Deno* d);

// Module API

typedef int deno_mod;

// Returns zero on error - check deno_last_exception().
deno_mod deno_mod_new(Deno* d, bool main, const char* name, const char* source);

size_t deno_mod_imports_len(Deno* d, deno_mod id);

// Returned pointer is valid for the lifetime of the Deno isolate "d".
const char* deno_mod_imports_get(Deno* d, deno_mod id, size_t index);

typedef deno_mod (*deno_resolve_cb)(void* user_data, const char* specifier,
                                    deno_mod referrer);

// If it succeeded deno_last_exception() will return NULL.
void deno_mod_instantiate(Deno* d, void* user_data, deno_mod id,
                          deno_resolve_cb cb);

// If it succeeded deno_last_exception() will return NULL.
void deno_mod_evaluate(Deno* d, void* user_data, deno_mod id);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
