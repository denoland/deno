// Copyright 2018-2026 the Deno authors. MIT license.
//
// A minimal NAPI-style module that references an OpenSSL symbol
// (`EVP_des_ede3_cbc`). It is intentionally linked *without* `-lcrypto`,
// so the symbol must be resolved at runtime from a globally-loaded
// libcrypto. This exercises the NAPI loader's runtime compatibility
// shim that pre-loads system OpenSSL libraries with `RTLD_GLOBAL` so
// that legacy Node.js native addons (e.g. NAN-based packages such as
// `nodegit`) can resolve the OpenSSL symbols they expect to find in
// the host binary.
//
// See: https://github.com/denoland/deno/issues/31730

#include <stdlib.h>

typedef struct napi_module {
  int nm_version;
  unsigned int nm_flags;
  const char* nm_filename;
  void* nm_register_func;
  const char* nm_modname;
  void* nm_priv;
  void* reserved[4];
} napi_module;

#ifdef _WIN32
#define NAPI_EXTERN __declspec(dllexport)
#define NAPI_CDECL __cdecl
#else
#define NAPI_EXTERN __attribute__((visibility("default")))
#define NAPI_CDECL
#endif

NAPI_EXTERN void NAPI_CDECL
napi_module_register(napi_module* mod);

// Declared but not linked. The symbol is provided by the system
// libcrypto, which the host (Deno) `dlopen`s with `RTLD_GLOBAL` before
// loading this module.
extern const void* EVP_des_ede3_cbc(void);

// Forces a reference to `EVP_des_ede3_cbc` that the dynamic linker must
// resolve before the call returns. The pointer is intentionally stored
// in a `volatile` to defeat any constant-folding that might let the
// compiler optimize the call away.
static void* init(void* env, void* exports) {
  (void)env;
  volatile const void* cipher = EVP_des_ede3_cbc();
  if (cipher == NULL) {
    // Defensive: should not happen if libcrypto is loaded — but if for
    // some reason `EVP_des_ede3_cbc` returns NULL, we want to fail
    // loudly rather than silently.
    abort();
  }
  return exports;
}

static napi_module _module = {
    1, 0, __FILE__, (void*)init, "openssl_compat", 0, {0},
};

static void __attribute__((constructor)) _register(void) {
  napi_module_register(&_module);
}
