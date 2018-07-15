// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include <stdio.h>
#include <stdlib.h>
#include <string>

#ifdef _WIN32
#include <direct.h>
#else
#include <unistd.h>
#endif

#include "deno.h"
#include "flatbuffer_builder.h"
#include "flatbuffers/flatbuffers.h"
#include "src/handlers.h"
#include "src/msg_generated.h"
#include "third_party/v8/src/base/logging.h"

namespace deno {

static char** global_argv;
static int global_argc;

// Sends StartRes message
void HandleStart(Deno* d, uint32_t cmd_id) {
  deno::FlatBufferBuilder builder;

  char cwdbuf[1024];
  // TODO(piscisaureus): support unicode on windows.
  getcwd(cwdbuf, sizeof(cwdbuf));
  auto start_cwd = builder.CreateString(cwdbuf);

  std::vector<flatbuffers::Offset<flatbuffers::String>> args;
  for (int i = 0; i < global_argc; ++i) {
    args.push_back(builder.CreateString(global_argv[i]));
  }

  auto start_argv = builder.CreateVector(args);
  auto start_msg = CreateStartRes(builder, start_cwd, start_argv);
  auto base = CreateBase(builder, cmd_id, 0, Any_StartRes, start_msg.Union());
  builder.Finish(base);
  deno_set_response(d, builder.ExportBuf());
}

void HandleCodeFetch(Deno* d, uint32_t cmd_id, const CodeFetch* msg) {
  auto module_specifier = msg->module_specifier()->c_str();
  auto containing_file = msg->containing_file()->c_str();
  printf("HandleCodeFetch module_specifier = %s containing_file = %s\n",
         module_specifier, containing_file);
  // Call into rust.
  handle_code_fetch(cmd_id, module_specifier, containing_file);
}

void MessagesFromJS(Deno* d, deno_buf buf) {
  flatbuffers::Verifier verifier(buf.data_ptr, buf.data_len);
  DCHECK(verifier.VerifyBuffer<Base>());

  auto base = flatbuffers::GetRoot<Base>(buf.data_ptr);
  auto cmd_id = base->cmdId();
  auto msg_type = base->msg_type();
  const char* msg_type_name = EnumNamesAny()[msg_type];
  printf("MessagesFromJS cmd_id = %d, msg_type = %d, msg_type_name = %s\n",
         cmd_id, msg_type, msg_type_name);
  switch (msg_type) {
    case Any_Start:
      HandleStart(d, base->cmdId());
      break;

    case Any_CodeFetch:
      HandleCodeFetch(d, base->cmdId(), base->msg_as_CodeFetch());
      break;

    case Any_NONE:
      CHECK(false && "Got message with msg_type == Any_NONE");
      break;

    default:
      printf("Unhandled message %s\n", msg_type_name);
      CHECK(false && "Unhandled message");
      break;
  }
}

int deno_main(int argc, char** argv) {
  deno_init();

  deno_set_flags(&argc, argv);
  global_argv = argv;
  global_argc = argc;

  Deno* d = deno_new(NULL, MessagesFromJS);
  bool r = deno_execute(d, "deno_main.js", "denoMain();");
  if (!r) {
    printf("%s\n", deno_last_exception(d));
    exit(1);
  }
  deno_delete(d);
  return 0;
}

}  // namespace deno

int main(int argc, char** argv) { return deno::deno_main(argc, argv); }
