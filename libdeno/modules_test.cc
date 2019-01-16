// Copyright 2018 the Deno authors. All rights reserved. MIT license.
#include "test.h"

static const char* a_src =
    "import { retb } from 'b.js'\n"
    "if (retb() != 'b') throw Error();\n"
    "libdeno.send(new Uint8Array([4, 2]));";

static const char* b_src = "export function retb() { return 'b' }";

TEST(ModulesTest, Resolution) {
  static int recv_count = 0;
  auto recv_cb = [](auto _, int req_id, auto buf, auto data_buf) {
    EXPECT_EQ(static_cast<size_t>(2), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 4);
    EXPECT_EQ(buf.data_ptr[1], 2);
    recv_count++;
  };

  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    auto d = reinterpret_cast<Deno*>(user_data);

    EXPECT_EQ(is_dynamic, 0);
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer_name, "a.js");

    EXPECT_EQ(deno_mod_get_state(d, referrer), DENO_MOD_UNINSTANCIATED);

    deno_mod b = deno_mod_new(d, d, "b.js", b_src);
    EXPECT_TRUE(b != 0);
    EXPECT_EQ(deno_mod_get_state(d, b), DENO_MOD_UNINSTANCIATED);

    deno_resolve(d, resolve_id, b);
    resolve_count++;
  };

  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb, resolve_cb});

  EXPECT_EQ(0, recv_count);
  EXPECT_EQ(0, resolve_count);

  deno_mod a = deno_mod_new(d, d, "a.js", a_src);
  EXPECT_NE(a, 0);

  EXPECT_EQ(0, recv_count);
  EXPECT_EQ(1, resolve_count);

  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_EVALUATED);

  EXPECT_EQ(1, recv_count);
  EXPECT_EQ(1, resolve_count);

  deno_delete(d);
}

TEST(ModulesTest, DelayedResolution) {
  static int recv_count = 0;
  auto recv_cb = [](auto _, int req_id, auto buf, auto data_buf) {
    EXPECT_EQ(static_cast<size_t>(2), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 4);
    EXPECT_EQ(buf.data_ptr[1], 2);
    recv_count++;
  };

  static std::vector<uint32_t> resolve_ids;
  static std::vector<std::string> resolve_specifiers;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    resolve_ids.push_back(resolve_id);
    resolve_specifiers.push_back(specifier);
    EXPECT_EQ(is_dynamic, 0);
    EXPECT_STREQ(referrer_name, "a.js");
  };

  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb, resolve_cb});

  EXPECT_EQ(0, recv_count);
  EXPECT_EQ(static_cast<size_t>(0), resolve_ids.size());
  EXPECT_EQ(static_cast<size_t>(0), resolve_specifiers.size());

  deno_mod a = deno_mod_new(d, d, "a.js",
                            "import { retb } from 'b.js'\n"
                            "import { retc } from 'c.js'\n"
                            "if (retb() != 'b') throw Error();\n"
                            "if (retc() != 'c') throw Error();\n"
                            "libdeno.send(new Uint8Array([4, 2]));");
  EXPECT_EQ(static_cast<size_t>(2), resolve_ids.size());
  EXPECT_EQ(static_cast<size_t>(2), resolve_specifiers.size());

  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_UNINSTANCIATED);

  EXPECT_STREQ(resolve_specifiers[0].c_str(), "b.js");
  deno_mod b =
      deno_mod_new(d, d, "b.js", "export function retb() { return 'b' }");
  deno_resolve(d, resolve_ids[0], b);

  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_UNINSTANCIATED);
  EXPECT_EQ(deno_mod_get_state(d, b), DENO_MOD_UNINSTANCIATED);

  EXPECT_STREQ(resolve_specifiers[1].c_str(), "c.js");
  deno_mod c =
      deno_mod_new(d, d, "c.js", "export function retc() { return 'c' }");
  deno_resolve(d, resolve_ids[1], c);

  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_UNINSTANCIATED);
  EXPECT_EQ(deno_mod_get_state(d, b), DENO_MOD_UNINSTANCIATED);
  EXPECT_EQ(deno_mod_get_state(d, c), DENO_MOD_UNINSTANCIATED);

  EXPECT_EQ(0, recv_count);
  deno_mod_evaluate(d, d, a);
  EXPECT_EQ(1, recv_count);

  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_EVALUATED);
  EXPECT_EQ(deno_mod_get_state(d, b), DENO_MOD_EVALUATED);
  EXPECT_EQ(deno_mod_get_state(d, c), DENO_MOD_EVALUATED);

  deno_delete(d);
}

TEST(ModulesTest, BuiltinModules) {
  static int count = 0;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer_name, "c.js");
    EXPECT_EQ(is_dynamic, 0);
    auto d = reinterpret_cast<Deno*>(user_data);

    deno_mod b = deno_mod_new(d, d, "b.js", b_src);
    EXPECT_NE(b, 0);
    EXPECT_EQ(deno_last_exception(d), nullptr);

    deno_resolve(d, resolve_id, b);
    EXPECT_EQ(deno_last_exception(d), nullptr);
    count++;
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  EXPECT_TRUE(deno_execute(
      d, d, "setup.js", "libdeno.builtinModules['deno'] = { foo: 'bar' }; \n"));
  EXPECT_EQ(count, 0);

  deno_mod c =
      deno_mod_new(d, d, "c.js",
                   "import { retb } from 'b.js'\n"
                   "import * as deno from 'deno'\n"
                   "if (retb() != 'b') throw Error('retb');\n"
                   // "   libdeno.print('deno ' + JSON.stringify(deno));\n"
                   "if (deno.foo != 'bar') throw Error('foo');\n");
  EXPECT_NE(c, 0);
  EXPECT_EQ(count, 1);
  EXPECT_EQ(deno_last_exception(d), nullptr);

  deno_mod_evaluate(d, d, c);
  EXPECT_EQ(count, 1);
  EXPECT_EQ(deno_last_exception(d), nullptr);

  deno_delete(d);
}

TEST(ModulesTest, BuiltinModules2) {
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    EXPECT_TRUE(false);  // We don't expect this to be called.
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  EXPECT_TRUE(deno_execute(
      d, d, "setup.js",
      "libdeno.builtinModules['builtin1'] = { foo: 'bar' }; \n"
      "libdeno.builtinModules['builtin2'] = { hello: 'world' }; \n"));
  EXPECT_EQ(deno_last_exception(d), nullptr);
  deno_mod c = deno_mod_new(d, d, "c.js",
                            "import * as b1 from 'builtin1'\n"
                            "import * as b2 from 'builtin2'\n"
                            "if (b1.foo != 'bar') throw Error('bad1');\n"
                            "if (b2.hello != 'world') throw Error('bad2');\n");
  EXPECT_NE(c, 0);
  EXPECT_EQ(deno_last_exception(d), nullptr);

  deno_mod_evaluate(d, d, c);
  EXPECT_EQ(deno_last_exception(d), nullptr);

  deno_delete(d);
}

TEST(ModulesTest, BuiltinModules3) {
  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    auto d = reinterpret_cast<Deno*>(user_data);

    EXPECT_EQ(is_dynamic, 0);
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer_name, "m.js");

    EXPECT_EQ(deno_mod_get_state(d, referrer), DENO_MOD_UNINSTANCIATED);

    deno_mod b = deno_mod_new(d, d, "b.js",
                              "import { foo } from 'builtin';\n"
                              "export function bar() { return foo }\n");
    EXPECT_NE(b, 0);
    EXPECT_EQ(deno_mod_get_state(d, b), DENO_MOD_UNINSTANCIATED);

    deno_resolve(d, resolve_id, b);
    resolve_count++;
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  EXPECT_TRUE(
      deno_execute(d, d, "setup.js",
                   "libdeno.builtinModules['builtin'] = { foo: 'bar' }; \n"));
  EXPECT_EQ(deno_last_exception(d), nullptr);
  deno_mod m = deno_mod_new(d, d, "m.js",
                            "import * as b1 from 'builtin'\n"
                            "import * as b2 from 'b.js'\n"
                            "if (b1.foo != 'bar') throw Error('bad1');\n"
                            "if (b2.bar() != 'bar') throw Error('bad2');\n");
  EXPECT_NE(m, 0);
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_EQ(1, resolve_count);

  deno_mod_evaluate(d, d, m);
  EXPECT_EQ(deno_last_exception(d), nullptr);

  deno_delete(d);
}

TEST(ModulesTest, ModuleSnapshot) {
  Deno* d1 = deno_new(deno_config{1, empty, empty, nullptr, nullptr});
  deno_mod x = deno_mod_new(d1, nullptr, "x.js",
                            "const globalEval = eval\n"
                            "const global = globalEval('this')\n"
                            "global.a = 1 + 2");
  EXPECT_NE(x, 0);
  deno_mod_evaluate(d1, d1, x);
  EXPECT_EQ(deno_last_exception(d1), nullptr);

  deno_buf test_snapshot = deno_get_snapshot(d1);
  deno_delete(d1);

  const char* y_src = "if (a != 3) throw Error('x');";

  deno_config config{0, test_snapshot, empty, nullptr, nullptr};
  Deno* d2 = deno_new(config);
  EXPECT_TRUE(deno_execute(d2, nullptr, "y.js", y_src));
  EXPECT_EQ(deno_last_exception(d2), nullptr);
  deno_delete(d2);

  Deno* d3 = deno_new(config);
  deno_mod m = deno_mod_new(d3, nullptr, "y.js", y_src);
  EXPECT_NE(m, 0);
  deno_mod_evaluate(d3, nullptr, m);
  EXPECT_EQ(deno_last_exception(d3), nullptr);

  deno_delete(d3);

  delete[] test_snapshot.data_ptr;
}

TEST(ModulesTest, ResolveOnly) {
  static int count = 0;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer_name, "a.js");
    count++;
    auto d = reinterpret_cast<Deno*>(user_data);
    deno_mod b = deno_mod_new(d, user_data, "b.js", b_src);
    EXPECT_NE(b, 0);
    deno_resolve(d, resolve_id, b);
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  // Code should not execute. If executed, the error would be thrown
  deno_mod_new(d, d, "a.js",
               "import { retb } from 'b.js'\n"
               "throw Error('unreachable');");
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

// We test here that if m depends on n and o, and those in turn depend on a
// common p, that everything works.
//    m
//   / \
//  n   o
//   \ /
//    p
TEST(ModulesTest, DiamondResolution) {
  static const char* m_src =
      "import { n } from 'n.js'\n"
      "import { o, p } from 'o.js'\n"
      "if (n() != 'n') throw Error();\n"
      "if (o() != 'o') throw Error();\n"
      "if (p() != 'p') throw Error();\n"
      "libdeno.send(new Uint8Array([5, 2]));";

  static const char* n_src =
      "import { p } from 'p.js'\n"
      "export function n() { return 'n' }\n"
      "if (p() != 'p') throw Error();\n";

  static const char* o_src =
      "import { p } from 'p.js'\n"
      "export function o() { return 'o' }\n"
      "if (p() != 'p') throw Error();\n"
      "export { p } from 'p.js'\n";

  static const char* p_src = "export function p() { return 'p' }\n";

  static deno_mod p = 0;

  static int recv_count = 0;
  auto recv_cb = [](auto _, int req_id, auto buf, auto data_buf) {
    EXPECT_EQ(static_cast<size_t>(2), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 5);
    EXPECT_EQ(buf.data_ptr[1], 2);
    recv_count++;
  };

  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    auto d = reinterpret_cast<Deno*>(user_data);
    EXPECT_EQ(is_dynamic, 0);

    resolve_count++;
    switch (resolve_count) {
      case 1: {
        EXPECT_STREQ(referrer_name, "m.js");
        EXPECT_STREQ(specifier, "n.js");
        deno_mod n = deno_mod_new(d, d, "n.js", n_src);
        EXPECT_NE(n, 0);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        deno_resolve(d, resolve_id, n);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        break;
      }

      case 2: {
        EXPECT_STREQ(referrer_name, "n.js");
        EXPECT_STREQ(specifier, "p.js");

        p = deno_mod_new(d, d, "p.js", p_src);
        EXPECT_NE(p, 0);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        deno_resolve(d, resolve_id, p);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        break;
      }

      case 3: {
        EXPECT_STREQ(referrer_name, "m.js");
        EXPECT_STREQ(specifier, "o.js");

        deno_mod o = deno_mod_new(d, d, "o.js", o_src);
        EXPECT_NE(o, 0);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        deno_resolve(d, resolve_id, o);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        break;
      }

      case 4: {
        EXPECT_STREQ(referrer_name, "o.js");
        EXPECT_STREQ(specifier, "p.js");
        EXPECT_NE(p, 0);
        deno_resolve(d, resolve_id, p);
        EXPECT_EQ(deno_last_exception(d), nullptr);
        break;
      }

      default:
        FAIL();
    }
  };

  Deno* d = deno_new(deno_config{0, empty, empty, recv_cb, resolve_cb});

  deno_mod m = deno_mod_new(d, d, "m.js", m_src);
  EXPECT_NE(m, 0);
  EXPECT_EQ(resolve_count, 4);
  EXPECT_EQ(recv_count, 0);

  deno_mod_evaluate(d, d, m);
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_EQ(resolve_count, 4);
  EXPECT_EQ(recv_count, 1);

  deno_delete(d);
}

TEST(ModulesTest, EvaluateFailure) {
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, nullptr});

  deno_mod a = deno_mod_new(d, d, "error_001.js",
                            "function foo() {\n"
                            "  throw Error('bad');\n"
                            "}\n"
                            "function bar() {\n"
                            "  foo();\n"
                            "}\n"
                            "bar();\n");
  EXPECT_NE(a, 0);
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_UNINSTANCIATED);

  deno_mod_evaluate(d, d, a);
  EXPECT_NE(deno_last_exception(d), nullptr);
  EXPECT_EQ(deno_mod_get_state(d, a), DENO_MOD_ERROR);

  deno_delete(d);
}

TEST(ModulesTest, ResolveFailure) {
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    auto d = reinterpret_cast<Deno*>(user_data);
    EXPECT_EQ(is_dynamic, 0);
    EXPECT_STREQ(referrer_name, "error_009_missing_js_module.js");
    EXPECT_STREQ(specifier, "./bad-module.js");
    deno_resolve(d, resolve_id, 0);
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});

  deno_mod m = deno_mod_new(d, d, "error_009_missing_js_module.js",
                            "import './bad-module.js';\n");
  EXPECT_NE(m, 0);
  EXPECT_EQ(deno_last_exception(d), nullptr);

  deno_mod_evaluate(d, d, m);
  EXPECT_NE(deno_last_exception(d), nullptr);

  deno_delete(d);
}

TEST(ModulesTest, SyntaxError) {
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, nullptr});

  deno_mod m = deno_mod_new(d, d, "error_syntax.js",
                            "\n(the following is a syntax error  ^^ ! )");
  EXPECT_EQ(m, 0);
  EXPECT_NE(deno_last_exception(d), nullptr);

  deno_delete(d);
}

TEST(ModulesTest, CircularImports) {
  static deno_mod circular1, circular2;
  static int resolve_count = 0;
  auto resolve_cb = [](void* user_data, uint32_t resolve_id, int is_dynamic,
                       const char* specifier, const char* referrer_name,
                       deno_mod referrer) {
    auto d = reinterpret_cast<Deno*>(user_data);
    EXPECT_EQ(is_dynamic, 0);
    resolve_count++;
    if (resolve_count == 1) {
      EXPECT_STREQ(referrer_name, "circular1.js");
      EXPECT_STREQ(specifier, "circular2.js");
      EXPECT_EQ(circular2, 0);
      circular2 = deno_mod_new(d, d, "circular2.js", "import 'circular1.js'");
      EXPECT_NE(circular2, 0);
      EXPECT_EQ(deno_last_exception(d), nullptr);
      deno_resolve(d, resolve_id, circular2);
    } else if (resolve_count == 2) {
      EXPECT_STREQ(referrer_name, "circular2.js");
      EXPECT_EQ(referrer, circular2);
      EXPECT_STREQ(specifier, "circular1.js");
      deno_resolve(d, resolve_id, circular1);
    } else {
      FAIL();
    }
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});

  circular1 = deno_mod_new(d, d, "circular1.js", "import 'circular2.js'");
  EXPECT_NE(circular1, 0);
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_EQ(resolve_count, 2);

  deno_mod_evaluate(d, d, circular1);
  printf("deno_last_exception(d) %s\n", deno_last_exception(d));
  EXPECT_EQ(deno_last_exception(d), nullptr);
  // EXPECT_EQ(deno_mod_get_state(d, circular1), DENO_MOD_EVALUATED);
  // EXPECT_EQ(deno_mod_get_state(d, circular2), DENO_MOD_EVALUATED);

  deno_delete(d);
}
