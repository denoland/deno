// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
#include "test.h"

TEST(LibDenoTest, InitializesCorrectly) {
  EXPECT_NE(snapshot.data_ptr, nullptr);
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "1 + 2"));
  deno_delete(d);
}

TEST(LibDenoTest, Snapshotter) {
  Deno* d1 = deno_new(deno_config{1, empty, empty, nullptr, nullptr});
  EXPECT_TRUE(deno_execute(d1, nullptr, "a.js", "a = 1 + 2"));
  deno_buf test_snapshot = deno_get_snapshot(d1);
  deno_delete(d1);

  Deno* d2 = deno_new(deno_config{0, test_snapshot, empty, nullptr, nullptr});
  EXPECT_TRUE(
      deno_execute(d2, nullptr, "b.js", "if (a != 3) throw Error('x');"));
  deno_delete(d2);

  delete[] test_snapshot.data_ptr;
}

TEST(LibDenoTest, CanCallFunction) {
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js",
                           "if (CanCallFunction() != 'foo') throw Error();"));
  deno_delete(d);
}

TEST(LibDenoTest, ErrorsCorrectly) {
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_FALSE(deno_execute(d, nullptr, "a.js", "throw Error()"));
  deno_delete(d);
}

deno_buf strbuf(const char* str) {
  auto len = strlen(str);

  deno_buf buf;
  buf.alloc_ptr = reinterpret_cast<uint8_t*>(strdup(str));
  buf.alloc_len = len + 1;
  buf.data_ptr = buf.alloc_ptr;
  buf.data_len = len;

  return buf;
}

// Same as strbuf but with null alloc_ptr.
deno_buf StrBufNullAllocPtr(const char* str) {
  auto len = strlen(str);
  deno_buf buf;
  buf.alloc_ptr = nullptr;
  buf.alloc_len = 0;
  buf.data_ptr = reinterpret_cast<uint8_t*>(strdup(str));
  buf.data_len = len;
  return buf;
}

void assert_null(deno_buf b) {
  EXPECT_EQ(b.alloc_ptr, nullptr);
  EXPECT_EQ(b.alloc_len, 0u);
  EXPECT_EQ(b.data_ptr, nullptr);
  EXPECT_EQ(b.data_len, 0u);
}

TEST(LibDenoTest, RecvReturnEmpty) {
  static int count = 0;
  auto recv_cb = [](auto _, int req_id, auto buf, auto data_buf) {
    assert_null(data_buf);
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 'a');
    EXPECT_EQ(buf.data_ptr[1], 'b');
    EXPECT_EQ(buf.data_ptr[2], 'c');
  };
  Deno* d = deno_new(deno_config{0, snapshot, empty, recv_cb, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "RecvReturnEmpty()"));
  EXPECT_EQ(count, 2);
  deno_delete(d);
}

TEST(LibDenoTest, RecvReturnBar) {
  static int count = 0;
  auto recv_cb = [](auto user_data, int req_id, auto buf, auto data_buf) {
    auto d = reinterpret_cast<Deno*>(user_data);
    assert_null(data_buf);
    count++;
    EXPECT_EQ(static_cast<size_t>(3), buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 'a');
    EXPECT_EQ(buf.data_ptr[1], 'b');
    EXPECT_EQ(buf.data_ptr[2], 'c');
    deno_respond(d, user_data, req_id, strbuf("bar"));
  };
  Deno* d = deno_new(deno_config{0, snapshot, empty, recv_cb, nullptr});
  EXPECT_TRUE(deno_execute(d, d, "a.js", "RecvReturnBar()"));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(LibDenoTest, DoubleRecvFails) {
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_FALSE(deno_execute(d, nullptr, "a.js", "DoubleRecvFails()"));
  deno_delete(d);
}

TEST(LibDenoTest, SendRecvSlice) {
  static int count = 0;
  auto recv_cb = [](auto user_data, int req_id, auto buf, auto data_buf) {
    auto d = reinterpret_cast<Deno*>(user_data);
    assert_null(data_buf);
    static const size_t alloc_len = 1024;
    size_t i = count++;
    // Check the size and offset of the slice.
    size_t data_offset = buf.data_ptr - buf.alloc_ptr;
    EXPECT_EQ(data_offset, i * 11);
    EXPECT_EQ(buf.data_len, alloc_len - i * 30);
    EXPECT_EQ(buf.alloc_len, alloc_len);
    // Check values written by the JS side.
    EXPECT_EQ(buf.data_ptr[0], 100 + i);
    EXPECT_EQ(buf.data_ptr[buf.data_len - 1], 100 - i);
    // Make copy of the backing buffer -- this is currently necessary
    // because deno_respond() takes ownership over the buffer, but we are
    // not given ownership of `buf` by our caller.
    uint8_t* alloc_ptr = reinterpret_cast<uint8_t*>(malloc(alloc_len));
    memcpy(alloc_ptr, buf.alloc_ptr, alloc_len);
    // Make a slice that is a bit shorter than the original.
    deno_buf buf2{alloc_ptr, alloc_len, alloc_ptr + data_offset,
                  buf.data_len - 19};
    // Place some values into the buffer for the JS side to verify.
    buf2.data_ptr[0] = 200 + i;
    buf2.data_ptr[buf2.data_len - 1] = 200 - i;
    // Send back.
    deno_respond(d, user_data, req_id, buf2);
  };
  Deno* d = deno_new(deno_config{0, snapshot, empty, recv_cb, nullptr});
  EXPECT_TRUE(deno_execute(d, d, "a.js", "SendRecvSlice()"));
  EXPECT_EQ(count, 5);
  deno_delete(d);
}

TEST(LibDenoTest, JSSendArrayBufferViewTypes) {
  static int count = 0;
  auto recv_cb = [](auto _, int req_id, auto buf, auto data_buf) {
    assert_null(data_buf);
    count++;
    size_t data_offset = buf.data_ptr - buf.alloc_ptr;
    EXPECT_EQ(data_offset, 2468u);
    EXPECT_EQ(buf.data_len, 1000u);
    EXPECT_EQ(buf.alloc_len, 4321u);
    EXPECT_EQ(buf.data_ptr[0], count);
  };
  Deno* d = deno_new(deno_config{0, snapshot, empty, recv_cb, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "JSSendArrayBufferViewTypes()"));
  EXPECT_EQ(count, 3);
  deno_delete(d);
}

TEST(LibDenoTest, TypedArraySnapshots) {
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "TypedArraySnapshots()"));
  deno_delete(d);
}

TEST(LibDenoTest, SnapshotBug) {
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "SnapshotBug()"));
  deno_delete(d);
}

TEST(LibDenoTest, GlobalErrorHandling) {
  Deno* d = deno_new(deno_config{0, snapshot, empty, nullptr, nullptr});
  EXPECT_FALSE(deno_execute(d, nullptr, "a.js", "GlobalErrorHandling()"));
  std::string expected =
      "{\"message\":\"Uncaught ReferenceError: notdefined is not defined\","
      "\"sourceLine\":\" "
      "notdefined()\",\"scriptResourceName\":\"helloworld.js\","
      "\"lineNumber\":3,\"startPosition\":3,\"endPosition\":4,\"errorLevel\":8,"
      "\"startColumn\":1,\"endColumn\":2,\"isSharedCrossOrigin\":false,"
      "\"isOpaque\":false,\"frames\":[{\"line\":3,\"column\":2,"
      "\"functionName\":\"\",\"scriptName\":\"helloworld.js\",\"isEval\":true,"
      "\"isConstructor\":false,\"isWasm\":false},";
  std::string actual(deno_last_exception(d), 0, expected.length());
  EXPECT_STREQ(expected.c_str(), actual.c_str());
  deno_delete(d);
}

TEST(LibDenoTest, DataBuf) {
  static int count = 0;
  static deno_buf data_buf_copy;
  auto recv_cb = [](auto _, int req_id, deno_buf buf, deno_buf data_buf) {
    count++;
    data_buf.data_ptr[0] = 4;
    data_buf.data_ptr[1] = 2;
    data_buf_copy = data_buf;
    EXPECT_EQ(2u, buf.data_len);
    EXPECT_EQ(2u, data_buf.data_len);
    EXPECT_EQ(buf.data_ptr[0], 1);
    EXPECT_EQ(buf.data_ptr[1], 2);
  };
  Deno* d = deno_new(deno_config{0, snapshot, empty, recv_cb, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "DataBuf()"));
  EXPECT_EQ(count, 1);
  // data_buf was subsequently changed in JS, let's check that our copy reflects
  // that.
  EXPECT_EQ(data_buf_copy.data_ptr[0], 9);
  EXPECT_EQ(data_buf_copy.data_ptr[1], 8);
  deno_delete(d);
}

TEST(LibDenoTest, CheckPromiseErrors) {
  static int count = 0;
  auto recv_cb = [](auto _, int req_id, auto buf, auto data_buf) { count++; };
  Deno* d = deno_new(deno_config{0, snapshot, empty, recv_cb, nullptr});
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "CheckPromiseErrors()"));
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_EQ(count, 1);
  // We caught the exception. So still no errors after calling
  // deno_check_promise_errors().
  deno_check_promise_errors(d);
  EXPECT_EQ(deno_last_exception(d), nullptr);
  deno_delete(d);
}

TEST(LibDenoTest, LastException) {
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, nullptr});
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_FALSE(deno_execute(d, nullptr, "a.js", "\n\nthrow Error('boo');\n\n"));
  EXPECT_STREQ(deno_last_exception(d),
               "{\"message\":\"Uncaught Error: boo\",\"sourceLine\":\"throw "
               "Error('boo');\",\"scriptResourceName\":\"a.js\",\"lineNumber\":"
               "3,\"startPosition\":8,\"endPosition\":9,\"errorLevel\":8,"
               "\"startColumn\":6,\"endColumn\":7,\"isSharedCrossOrigin\":"
               "false,\"isOpaque\":false,\"frames\":[{\"line\":3,\"column\":7,"
               "\"functionName\":\"\",\"scriptName\":\"a.js\",\"isEval\":false,"
               "\"isConstructor\":false,\"isWasm\":false}]}");
  deno_delete(d);
}

TEST(LibDenoTest, EncodeErrorBug) {
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, nullptr});
  EXPECT_EQ(deno_last_exception(d), nullptr);
  EXPECT_FALSE(deno_execute(d, nullptr, "a.js", "eval('a')"));
  EXPECT_STREQ(
      deno_last_exception(d),
      "{\"message\":\"Uncaught ReferenceError: a is not "
      "defined\",\"sourceLine\":\"a\",\"lineNumber\":1,\"startPosition\":0,"
      "\"endPosition\":1,\"errorLevel\":8,\"startColumn\":0,\"endColumn\":1,"
      "\"isSharedCrossOrigin\":false,\"isOpaque\":false,\"frames\":[{\"line\":"
      "1,\"column\":1,\"functionName\":\"\",\"scriptName\":\"<unknown>\","
      "\"isEval\":true,\"isConstructor\":false,\"isWasm\":false},{\"line\":1,"
      "\"column\":1,\"functionName\":\"\",\"scriptName\":\"a.js\",\"isEval\":"
      "false,\"isConstructor\":false,\"isWasm\":false}]}");
  deno_delete(d);
}

TEST(LibDenoTest, Shared) {
  uint8_t s[] = {0, 1, 2};
  deno_buf shared = {nullptr, 0, s, 3};
  Deno* d = deno_new(deno_config{0, snapshot, shared, nullptr, nullptr});
  EXPECT_TRUE(deno_execute(d, nullptr, "a.js", "Shared()"));
  EXPECT_EQ(s[0], 42);
  EXPECT_EQ(s[1], 43);
  EXPECT_EQ(s[2], 44);
  deno_delete(d);
}

static const char* mod_a =
    "import { retb } from 'b.js'\n"
    "if (retb() != 'b') throw Error();";

static const char* mod_b = "export function retb() { return 'b' }";

TEST(LibDenoTest, ModuleResolution) {
  static int count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       const char* referrer) {
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer, "a.js");
    count++;
    auto d = reinterpret_cast<Deno*>(user_data);
    deno_resolve_ok(d, "b.js", mod_b);
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  EXPECT_TRUE(deno_execute_mod(d, d, "a.js", mod_a, false));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(LibDenoTest, ModuleResolutionFail) {
  static int count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       const char* referrer) {
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer, "a.js");
    count++;
    // Do not call deno_resolve_ok();
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  EXPECT_FALSE(deno_execute_mod(d, d, "a.js", mod_a, false));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(LibDenoTest, ModuleSnapshot) {
  Deno* d1 = deno_new(deno_config{1, empty, empty, nullptr, nullptr});
  EXPECT_TRUE(deno_execute_mod(d1, nullptr, "x.js",
                               "const globalEval = eval\n"
                               "const global = globalEval('this')\n"
                               "global.a = 1 + 2",
                               0));
  deno_buf test_snapshot = deno_get_snapshot(d1);
  deno_delete(d1);

  const char* y_src = "if (a != 3) throw Error('x');";

  deno_config config{0, test_snapshot, empty, nullptr, nullptr};
  Deno* d2 = deno_new(config);
  EXPECT_TRUE(deno_execute(d2, nullptr, "y.js", y_src));
  deno_delete(d2);

  Deno* d3 = deno_new(config);
  EXPECT_TRUE(deno_execute_mod(d3, nullptr, "y.js", y_src, false));
  deno_delete(d3);

  delete[] test_snapshot.data_ptr;
}

TEST(LibDenoTest, ModuleResolveOnly) {
  static int count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       const char* referrer) {
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer, "a.js");
    count++;
    auto d = reinterpret_cast<Deno*>(user_data);
    deno_resolve_ok(d, "b.js", mod_b);
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  // Code should not execute. If executed, the error would be thrown
  EXPECT_TRUE(deno_execute_mod(d, d, "a.js",
                               "import { retb } from 'b.js'\n"
                               "throw Error('unreachable');",
                               true));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}

TEST(LibDenoTest, BuiltinModules) {
  static int count = 0;
  auto resolve_cb = [](void* user_data, const char* specifier,
                       const char* referrer) {
    EXPECT_STREQ(specifier, "b.js");
    EXPECT_STREQ(referrer, "c.js");
    count++;
    auto d = reinterpret_cast<Deno*>(user_data);
    deno_resolve_ok(d, "b.js", mod_b);
  };
  Deno* d = deno_new(deno_config{0, empty, empty, nullptr, resolve_cb});
  EXPECT_TRUE(deno_execute(
      d, d, "setup.js", "libdeno.builtinModules['deno'] = { foo: 'bar' }; \n"));
  EXPECT_EQ(count, 0);
  EXPECT_TRUE(
      deno_execute_mod(d, d, "c.js",
                       "import { retb } from 'b.js'\n"
                       "import * as deno from 'deno'\n"
                       "if (retb() != 'b') throw Error('retb');\n"
                       // "   libdeno.print('deno ' + JSON.stringify(deno));\n"
                       "if (deno.foo != 'bar') throw Error('foo');\n",
                       false));
  EXPECT_EQ(count, 1);
  deno_delete(d);
}
