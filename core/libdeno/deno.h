// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
#ifndef DENO_H_
#define DENO_H_

#include <stddef.h>
#include <stdint.h>

#include "buffer.h"

// Neither Rust nor Go support calling directly into C++ functions, therefore
// the public interface to libdeno is done in C.
#ifdef __cplusplus
extern "C" {
#endif

typedef deno::PinnedBuf::Raw deno_pinned_buf;

// Data that gets transmitted.
typedef struct {
  uint8_t* data_ptr;
  size_t data_len;
} deno_buf;

typedef struct {
  uint8_t* data_ptr;
  size_t data_len;
} deno_snapshot;

typedef struct deno_s Deno;

typedef uint32_t deno_op_id;

// A callback to receive a message from a Deno.core.send() javascript call.
// control_buf is valid for only for the lifetime of this callback.
// data_buf is valid until deno_respond() is called.
//
// op_id corresponds to the first argument of Deno.core.send().
// op_id is an extra user-defined integer valued which is not interpreted by
// libdeno.
//
// control_buf corresponds to the second argument of Deno.core.send().
//
// zero_copy_buf corresponds to the third argument of Deno.core.send().
// The user must call deno_pinned_buf_delete on each zero_copy_buf received.
typedef void (*deno_recv_cb)(void* user_data, deno_op_id op_id,
                             deno_buf control_buf,
                             deno_pinned_buf zero_copy_buf);

typedef int deno_dyn_import_id;
// Called when dynamic import is called in JS: import('foo')
// Embedder must call deno_dyn_import_done() with the specified id and
// the module.
typedef void (*deno_dyn_import_cb)(void* user_data, const char* specifier,
                                   const char* referrer, deno_dyn_import_id id);

void deno_init();
const char* deno_v8_version();
void deno_set_v8_flags(int* argc, char** argv);

typedef struct {
  int will_snapshot;            // Default 0. If calling deno_snapshot_new 1.
  deno_snapshot load_snapshot;  // A startup snapshot to use.
  deno_buf shared;              // Shared buffer to be mapped to libdeno.shared
  deno_recv_cb recv_cb;         // Maps to Deno.core.send() calls.
  deno_dyn_import_cb dyn_import_cb;
} deno_config;

// Create a new deno isolate.
// Warning: If config.will_snapshot is set, deno_snapshot_new() must be called
// or an error will result.
Deno* deno_new(deno_config config);
void deno_delete(Deno* d);

// Generate a snapshot. The resulting buf can be used in as the load_snapshot
// member in deno_confg.
// When calling this function, the caller must have created the isolate "d" with
// "will_snapshot" set to 1.
// The caller must free the returned data with deno_snapshot_delete().
deno_snapshot deno_snapshot_new(Deno* d);

// Only for use with data returned from deno_snapshot_new.
void deno_snapshot_delete(deno_snapshot);

void deno_lock(Deno* d);
void deno_unlock(Deno* d);

// Compile and execute a traditional JavaScript script that does not use
// module import statements.
// If it succeeded deno_last_exception() will return NULL.
void deno_execute(Deno* d, void* user_data, const char* js_filename,
                  const char* js_source);

// deno_respond sends one message back for every deno_recv_cb made.
//
// If this is called during deno_recv_cb, the issuing Deno.core.send() in
// javascript will synchronously return the specified buf as an ArrayBuffer (or
// null if buf is empty).
//
// If this is called after deno_recv_cb has returned, the deno_respond
// will call into the JS callback specified by Deno.core.recv().
//
// (Ideally, but not currently: After calling deno_respond(), the caller no
// longer owns `buf` and must not use it; deno_respond() is responsible for
// releasing its memory.)
//
// op_id is an extra user-defined integer valued which is not currently
// interpreted by libdeno. But it should probably correspond to the op_id in
// deno_recv_cb.
//
// If a JS exception was encountered, deno_last_exception() will be non-NULL.
void deno_respond(Deno* d, void* user_data, deno_op_id op_id, deno_buf buf);

void deno_throw_exception(Deno* d, const char* text);

// consumes zero_copy
void deno_pinned_buf_delete(deno_pinned_buf* buf);

void deno_check_promise_errors(Deno* d);

// Returns a cstring pointer to the exception.
// Rust side must NOT assert ownership.
const char* deno_last_exception(Deno* d);

// Clears last exception.
// Rust side must NOT hold pointer to exception string when called.
void deno_clear_last_exception(Deno* d_);

void deno_terminate_execution(Deno* d);

void deno_run_microtasks(Deno* d, void* user_data);
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

// Call exactly once for every deno_dyn_import_cb.
// Note this call will execute JS.
// Either mod_id is zero and error_str is not null OR mod_id is valid and
// error_str is null.
// TODO(ry) The errors arising from dynamic import are not exactly the same as
// those arising from ops in Deno.
void deno_dyn_import_done(Deno* d, void* user_data, deno_dyn_import_id id,
                          deno_mod mod_id, const char* error_str);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
