// Copyright 2018-2025 the Deno authors. MIT license.

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

#if defined(_MSC_VER)
#if defined(__cplusplus)
#define NAPI_C_CTOR(fn)                                                        \
  static void NAPI_CDECL fn(void);                                             \
  namespace {                                                                  \
  struct fn##_ {                                                               \
    fn##_() { fn(); }                                                          \
  } fn##_v_;                                                                   \
  }                                                                            \
  static void NAPI_CDECL fn(void)
#else  // !defined(__cplusplus)
#pragma section(".CRT$XCU", read)
// The NAPI_C_CTOR macro defines a function fn that is called during CRT
// initialization.
// C does not support dynamic initialization of static variables and this code
// simulates C++ behavior. Exporting the function pointer prevents it from being
// optimized. See for details:
// https://docs.microsoft.com/en-us/cpp/c-runtime-library/crt-initialization?view=msvc-170
#define NAPI_C_CTOR(fn)                                                        \
  static void NAPI_CDECL fn(void);                                             \
  __declspec(dllexport, allocate(".CRT$XCU")) void(NAPI_CDECL * fn##_)(void) = \
      fn;                                                                      \
  static void NAPI_CDECL fn(void)
#endif  // defined(__cplusplus)
#else
#define NAPI_C_CTOR(fn)                                                        \
  static void fn(void) __attribute__((constructor));                           \
  static void fn(void)
#endif

#define NAPI_MODULE_TEST(modname, regfunc)                                     \
  static napi_module _module = {                                               \
      1,                                                                       \
      0,                                                                       \
      __FILE__,                                                                \
      regfunc,                                                                 \
      #modname,                                                                \
      0,                                                                       \
      {0},                                                                     \
  };                                                                           \
  NAPI_C_CTOR(_register_##modname) { napi_module_register(&_module); }         \

void* init(void* env __attribute__((unused)), void* exports) {
  return exports;
}

NAPI_MODULE_TEST(TEST_NAPI_MODULE_NAME, init)
