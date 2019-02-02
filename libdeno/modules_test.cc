// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "test.h"

static int exec_count = 0;
void recv_cb(void* user_data, int req_id, deno_buf buf, deno_buf data_buf) {
  // We use this to check that scripts have executed.
  EXPECT_EQ(1u, buf.data_len);
  EXPECT_EQ(buf.data_ptr[0], 4);
  exec_count++;
}

TEST(ModulesTest, Resolution) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  static deno_mod a = deno_mod_new(d, "a.js",
                                   "import { b } from 'b.js'\n"
                                   "if (b() != 'b') throw Error();\n"
                                   "libdeno.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  const char* b_src = "export function b() { return 'b' }";
  static deno_mod b = deno_mod_new(d, "b.js", b_src);
  EXPECT_NE(b, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  EXPECT_EQ(0, exec_count);

  EXPECT_EQ(1u, deno_mod_imports_len(d, a));
  EXPECT_EQ(0u, deno_mod_imports_len(d, b));

  EXPECT_STREQ("b.js", deno_mod_imports_get(d, a, 0));
  EXPECT_EQ(nullptr, deno_mod_imports_get(d, a, 1));
  EXPECT_EQ(nullptr, deno_mod_imports_get(d, b, 0));

  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       deno_mod referrer) {
    EXPECT_EQ(referrer, a);
    EXPECT_STREQ(specifier, "b.js");
    resolve_count++;
    return b;
  };

  deno_mod_instantiate(d, d, b, resolve_cb);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(0, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_mod_instantiate(d, d, a, resolve_cb);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(1, exec_count);

  deno_delete(d);
}

TEST(ModulesTest, BuiltinModules) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  deno_execute(d, d, "setup.js",
               "libdeno.builtinModules['deno'] = { foo: 'bar' };");
  EXPECT_EQ(nullptr, deno_last_exception(d));

  static deno_mod a =
      deno_mod_new(d, "a.js",
                   "import { b } from 'b.js'\n"
                   "import * as deno from 'deno'\n"
                   "if (b() != 'b') throw Error('b');\n"
                   "if (deno.foo != 'bar') throw Error('foo');\n"
                   "libdeno.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  const char* b_src = "export function b() { return 'b' }";
  static deno_mod b = deno_mod_new(d, "b.js", b_src);
  EXPECT_NE(b, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  EXPECT_EQ(0, exec_count);

  EXPECT_EQ(2u, deno_mod_imports_len(d, a));
  EXPECT_EQ(0u, deno_mod_imports_len(d, b));

  EXPECT_STREQ("b.js", deno_mod_imports_get(d, a, 0));
  EXPECT_STREQ("deno", deno_mod_imports_get(d, a, 1));
  EXPECT_EQ(nullptr, deno_mod_imports_get(d, a, 2));
  EXPECT_EQ(nullptr, deno_mod_imports_get(d, b, 0));

  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       deno_mod referrer) {
    EXPECT_EQ(referrer, a);
    EXPECT_STREQ(specifier, "b.js");
    resolve_count++;
    return b;
  };

  deno_mod_instantiate(d, d, b, resolve_cb);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(0, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_mod_instantiate(d, d, a, resolve_cb);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(1, exec_count);

  deno_delete(d);
}

TEST(ModulesTest, BuiltinModules2) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  deno_execute(d, d, "setup.js",
               "libdeno.builtinModules['builtin1'] = { foo: 'bar' }; \n"
               "libdeno.builtinModules['builtin2'] = { hello: 'world' }; \n");
  EXPECT_EQ(nullptr, deno_last_exception(d));

  static deno_mod a =
      deno_mod_new(d, "a.js",
                   "import * as b1 from 'builtin1'\n"
                   "import * as b2 from 'builtin2'\n"
                   "if (b1.foo != 'bar') throw Error('bad1');\n"
                   "if (b2.hello != 'world') throw Error('bad2');\n"
                   "libdeno.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  EXPECT_EQ(2u, deno_mod_imports_len(d, a));
  EXPECT_STREQ("builtin1", deno_mod_imports_get(d, a, 0));
  EXPECT_STREQ("builtin2", deno_mod_imports_get(d, a, 1));

  deno_mod_instantiate(d, d, a, nullptr);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(0, exec_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, exec_count);

  deno_delete(d);
}

TEST(ModulesTest, BuiltinModules3) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  deno_execute(d, d, "setup.js",
               "libdeno.builtinModules['builtin'] = { foo: 'bar' };");
  EXPECT_EQ(nullptr, deno_last_exception(d));

  static deno_mod a =
      deno_mod_new(d, "a.js",
                   "import * as b1 from 'builtin'\n"
                   "import * as b2 from 'b.js'\n"
                   "if (b1.foo != 'bar') throw Error('bad1');\n"
                   "if (b2.bar() != 'bar') throw Error('bad2');\n"
                   "libdeno.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  EXPECT_EQ(2u, deno_mod_imports_len(d, a));
  EXPECT_STREQ("builtin", deno_mod_imports_get(d, a, 0));
  EXPECT_STREQ("b.js", deno_mod_imports_get(d, a, 1));

  static deno_mod b = deno_mod_new(d, "b.js",
                                   "import { foo } from 'builtin';\n"
                                   "export function bar() { return foo }\n");
  EXPECT_NE(b, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       deno_mod referrer) {
    EXPECT_EQ(referrer, a);
    EXPECT_STREQ(specifier, "b.js");
    resolve_count++;
    return b;
  };

  deno_mod_instantiate(d, d, a, resolve_cb);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_mod_instantiate(d, d, b, resolve_cb);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, exec_count);

  deno_delete(d);
}

TEST(ModulesTest, ResolutionError) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  static deno_mod a = deno_mod_new(d, "a.js",
                                   "import 'bad'\n"
                                   "libdeno.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  EXPECT_EQ(0, exec_count);

  EXPECT_EQ(1u, deno_mod_imports_len(d, a));
  EXPECT_STREQ("bad", deno_mod_imports_get(d, a, 0));

  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       deno_mod referrer) {
    EXPECT_EQ(referrer, a);
    EXPECT_STREQ(specifier, "bad");
    resolve_count++;
    return 0;
  };

  deno_mod_instantiate(d, d, a, resolve_cb);
  EXPECT_NE(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, resolve_count);
  EXPECT_EQ(0, exec_count);

  deno_delete(d);
}

TEST(ModulesTest, ImportMetaUrl) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  static deno_mod a =
      deno_mod_new(d, "a.js",
                   "if ('a.js' != import.meta.url) throw 'hmm'\n"
                   "libdeno.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_mod_instantiate(d, d, a, nullptr);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(0, exec_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, exec_count);
}
