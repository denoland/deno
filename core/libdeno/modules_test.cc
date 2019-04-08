// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "test.h"

static int exec_count = 0;
void recv_cb(void* user_data, deno_buf buf, deno_buf zero_copy_buf) {
  // We use this to check that scripts have executed.
  EXPECT_EQ(1u, buf.data_len);
  EXPECT_EQ(buf.data_ptr[0], 4);
  EXPECT_EQ(zero_copy_buf.zero_copy_id, 0u);
  EXPECT_EQ(zero_copy_buf.data_ptr, nullptr);
  exec_count++;
}

TEST(ModulesTest, Resolution) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty_snapshot, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  static deno_mod a = deno_mod_new(d, true, "a.js",
                                   "import { b } from 'b.js'\n"
                                   "if (b() != 'b') throw Error();\n"
                                   "Deno.core.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  const char* b_src = "export function b() { return 'b' }";
  static deno_mod b = deno_mod_new(d, false, "b.js", b_src);
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

TEST(ModulesTest, ResolutionError) {
  exec_count = 0;  // Reset
  Deno* d = deno_new(deno_config{0, empty_snapshot, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  static deno_mod a = deno_mod_new(d, true, "a.js",
                                   "import 'bad'\n"
                                   "Deno.core.send(new Uint8Array([4]));");
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
  Deno* d = deno_new(deno_config{0, empty_snapshot, empty, recv_cb});
  EXPECT_EQ(0, exec_count);

  static deno_mod a =
      deno_mod_new(d, true, "a.js",
                   "if ('a.js' != import.meta.url) throw 'hmm'\n"
                   "Deno.core.send(new Uint8Array([4]));");
  EXPECT_NE(a, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_mod_instantiate(d, d, a, nullptr);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(0, exec_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(nullptr, deno_last_exception(d));
  EXPECT_EQ(1, exec_count);
}

TEST(ModulesTest, ImportMetaMain) {
  Deno* d = deno_new(deno_config{0, empty_snapshot, empty, recv_cb});

  const char* throw_not_main_src = "if (!import.meta.main) throw 'err'";
  static deno_mod throw_not_main =
      deno_mod_new(d, true, "a.js", throw_not_main_src);
  EXPECT_NE(throw_not_main, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_mod_instantiate(d, d, throw_not_main, nullptr);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_mod_evaluate(d, d, throw_not_main);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  const char* throw_main_src = "if (import.meta.main) throw 'err'";
  static deno_mod throw_main = deno_mod_new(d, false, "b.js", throw_main_src);
  EXPECT_NE(throw_main, 0);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_mod_instantiate(d, d, throw_main, nullptr);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_mod_evaluate(d, d, throw_main);
  EXPECT_EQ(nullptr, deno_last_exception(d));

  deno_delete(d);
}
