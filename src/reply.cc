// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

// When Rust Flatbuffer support is complete this file should be ported
// to Rust and removed: https://github.com/google/flatbuffers/pull/3894
#include <vector>
// getcwd
#ifdef _WIN32
#include <direct.h>
#else
#include <unistd.h>
#endif

#include "flatbuffers/flatbuffers.h"
#include "src/deno.h"
#include "src/flatbuffer_builder.h"
#include "src/handlers.h"
#include "src/internal.h"
#include "src/msg_generated.h"
#include "src/reply.h"
#include "third_party/v8/src/base/logging.h"

extern "C" {

void deno_reply_error(Deno* d, uint32_t cmd_id, const char* error_msg) {
  // printf("deno_reply_error: %s\n", error_msg);
  deno::FlatBufferBuilder builder;
  auto error_msg_ = error_msg ? builder.CreateString(error_msg) : 0;
  auto base = deno::CreateBase(builder, cmd_id, error_msg_);
  builder.Finish(base);
  deno_set_response(d, builder.ExportBuf());
}

void deno_reply_null(Deno* d, uint32_t cmd_id) {
  deno_reply_error(d, cmd_id, nullptr);
}

void deno_reply_code_fetch(Deno* d, uint32_t cmd_id, const char* module_name,
                           const char* filename, const char* source_code,
                           const char* output_code) {
  deno::FlatBufferBuilder builder;
  auto module_name_ = builder.CreateString(module_name);
  auto filename_ = builder.CreateString(filename);
  auto source_code_ = builder.CreateString(source_code);
  auto output_code_ = builder.CreateString(output_code);
  auto code_fetch_res = deno::CreateCodeFetchRes(
      builder, module_name_, filename_, source_code_, output_code_);
  auto base = deno::CreateBase(builder, cmd_id, 0, deno::Any_CodeFetchRes,
                               code_fetch_res.Union());
  builder.Finish(base);
  deno_set_response(d, builder.ExportBuf());
}

void deno_reply_start(Deno* d, uint32_t cmd_id, int argc, char* argv[],
                      char* cwd) {
  deno::FlatBufferBuilder builder;
  auto start_cwd = builder.CreateString(cwd);
  std::vector<flatbuffers::Offset<flatbuffers::String>> args;
  for (int i = 0; i < argc; ++i) {
    args.push_back(builder.CreateString(argv[i]));
  }
  auto start_argv = builder.CreateVector(args);
  auto start_msg = deno::CreateStartRes(builder, start_cwd, start_argv);
  auto base = deno::CreateBase(builder, cmd_id, 0, deno::Any_StartRes,
                               start_msg.Union());
  builder.Finish(base);
  deno_set_response(d, builder.ExportBuf());
}

void deno_handle_msg_from_js(Deno* d, deno_buf buf) {
  flatbuffers::Verifier verifier(buf.data_ptr, buf.data_len);
  DCHECK(verifier.VerifyBuffer<deno::Base>());

  auto base = flatbuffers::GetRoot<deno::Base>(buf.data_ptr);
  auto cmd_id = base->cmdId();
  auto msg_type = base->msg_type();
  const char* msg_type_name = deno::EnumNamesAny()[msg_type];
  switch (msg_type) {
    case deno::Any_Start: {
      char cwdbuf[1024];
      // TODO(piscisaureus): support unicode on windows.
      getcwd(cwdbuf, sizeof(cwdbuf));
      deno_reply_start(d, cmd_id, deno_argc(), deno_argv(), cwdbuf);
      break;
    }

    case deno::Any_CodeFetch: {
      auto msg = base->msg_as_CodeFetch();
      auto module_specifier = msg->module_specifier()->c_str();
      auto containing_file = msg->containing_file()->c_str();
      handle_code_fetch(d, cmd_id, module_specifier, containing_file);
      break;
    }

    case deno::Any_CodeCache: {
      // TODO(ry) Call into rust.
      /*
      auto filename = msg->filename()->c_str();
      auto source_code = msg->source_code()->c_str();
      auto output_code = msg->output_code()->c_str();
      printf(
          "HandleCodeCache (not implemeneted) filename %s source_code %s "
          "output_code %s\n",
          filename, source_code, output_code);
      */
      break;
    }

    case deno::Any_NONE:
      CHECK(false && "Got message with msg_type == Any_NONE");
      break;

    default:
      printf("Unhandled message %s\n", msg_type_name);
      CHECK(false && "Unhandled message");
      break;
  }
}

}  // extern "C"
