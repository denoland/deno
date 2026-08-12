// Copyright 2018-2026 the Deno authors. MIT license.

// A minimal *legacy* V8/nan style native addon. Unlike an N-API addon it
// registers itself through `node_module_register` (the `NODE_MODULE` macro)
// with `nm_version` set to `NODE_MODULE_VERSION` rather than the Node-API
// version (1). Deno does not support the legacy ABI and must reject this with a
// clear error instead of crashing. See denoland/deno#26656.

typedef struct node_module {
  int nm_version;
  unsigned int nm_flags;
  const char* nm_filename;
  void* nm_register_func;
  const char* nm_modname;
  void* nm_priv;
  void* reserved[4];
} node_module;

#ifdef _WIN32
#define NODE_CDECL __cdecl
#else
#define NODE_CDECL
#endif

extern void NODE_CDECL node_module_register(node_module* mod);

#if defined(_MSC_VER)
#pragma section(".CRT$XCU", read)
#define NODE_C_CTOR(fn)                                                        \
  static void NODE_CDECL fn(void);                                             \
  __declspec(dllexport, allocate(".CRT$XCU")) void(NODE_CDECL * fn##_)(void) = \
      fn;                                                                      \
  static void NODE_CDECL fn(void)
#else
#define NODE_C_CTOR(fn)                                                        \
  static void fn(void) __attribute__((constructor));                          \
  static void fn(void)
#endif

// A realistic NODE_MODULE_VERSION value (Node.js 22). The exact number does not
// matter as long as it is not the Node-API version (1).
#define NODE_MODULE_VERSION 127

static void init(void* exports, void* module, void* priv) {
  (void)exports;
  (void)module;
  (void)priv;
}

static node_module _module = {
    NODE_MODULE_VERSION,
    0,
    __FILE__,
    init,
    "legacy_test",
    0,
    {0},
};

NODE_C_CTOR(_register_legacy_test) {
  node_module_register(&_module);
}
