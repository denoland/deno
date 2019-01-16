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
} deno_buf;

typedef struct deno_s Deno;

// An "EcmaScript" module id. 0 is a bad value.
typedef int deno_mod;

// A callback to receive a message from a libdeno.send() javascript call.
// control_buf is valid for only for the lifetime of this callback.
// data_buf is valid until deno_respond() is called.
typedef void (*deno_recv_cb)(void* user_data, int32_t req_id,
                             deno_buf control_buf, deno_buf data_buf);

typedef uint32_t deno_resolve_id;

// Called during deno_mod_new for static imports.
// Called during deno_mod_evaluate for dynamic imports.
// is_dynamic: 0 = static import, 1 = dynamic import.
//
// The receiver must call deno_resolve() for each resolve_id received in this
// way. If a resolution error occurred, call deno_resolve() with child_id = 0.
typedef void (*deno_resolve_cb)(void* user_data, deno_resolve_id resolve_id,
                                int is_dynamic, const char* specifier,
                                const char* referrer, deno_mod referrer_id);

void deno_init();
const char* deno_v8_version();
void deno_set_v8_flags(int* argc, char** argv);

typedef struct {
  int will_snapshot;           // Default 0. If calling deno_get_snapshot 1.
  deno_buf load_snapshot;      // Optionally: A deno_buf from deno_get_snapshot.
  deno_buf shared;             // Shared buffer to be mapped to libdeno.shared
  deno_recv_cb recv_cb;        // Maps to libdeno.send() calls.
  deno_resolve_cb resolve_cb;  // Implement to use ES modules.
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

// Returns non-zero deno_mod module id on success.
// Re-entrant: deno_resolve_cb will be called during this invocation.
// On failure, get error text with deno_last_exception().
deno_mod deno_mod_new(Deno* d, void* user_data, const char* filename,
                      const char* source);

typedef enum {
  DENO_MOD_ERROR = 0,
  DENO_MOD_UNINSTANCIATED = 1,
  DENO_MOD_INSTANCIATED = 2,
  DENO_MOD_EVALUATED = 3,
} deno_mod_state;
deno_mod_state deno_mod_get_state(Deno* d, deno_mod id);

// The module must have state DENO_MOD_INSTANCIATED.
// Only call this on the main module.
// Child modules only should have only deno_mod_instantiate() called on them.
// The state of the module  will be DENO_MOD_ERROR or DENO_MOD_EVALUATED.
// Get error text with deno_last_exception().
//
// This function is re-entrant. That is, it may issue resolve_cb callbacks
// during its invocation for dynamic module imports.
void deno_mod_evaluate(Deno* d, void* user_data, deno_mod id);

// Call this for every invocation of deno_resolve_cb with the same resolve_id.
//
// child should be the output of a call to deno_mod_new().
//
// This function is re-entrant. That is, it may issue resolve_cb callbacks
// during its invocation.
//
// deno_resolve() is not thread safe. It must be called on the V8 thread.
//
// The state of the referrer may change after this call.
// Get error text with deno_last_exception().
void deno_resolve(Deno* d, deno_resolve_id resolve_id, deno_mod child);

#ifdef __cplusplus
}  // extern "C"
#endif
#endif  // DENO_H_
