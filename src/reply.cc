// Copyright 2018 the Deno authors. All rights reserved. MIT license.

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
      auto msg = base->msg_as_CodeCache();
      auto filename = msg->filename()->c_str();
      auto source_code = msg->source_code()->c_str();
      auto output_code = msg->output_code()->c_str();
      handle_code_cache(d, cmd_id, filename, source_code, output_code);
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
