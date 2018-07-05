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

#include "flatbuffers/flatbuffers.h"
#include "deno.h"
#include "src/msg_generated.h"
#include "third_party/v8/src/base/logging.h"

static char** global_argv;
static int global_argc;

void MessagesFromJS(Deno* d, const char* channel, deno_buf buf) {
  printf("MessagesFromJS %s\n", channel);

  flatbuffers::FlatBufferBuilder builder;

  char cwdbuf[1024];
  // TODO(piscisaureus): support unicode on windows.
  getcwd(cwdbuf, sizeof(cwdbuf));
  auto start_cwd = builder.CreateString(cwdbuf);

  std::vector<flatbuffers::Offset<flatbuffers::String>> args;
  for (int i = 0; i < global_argc; ++i) {
    args.push_back(builder.CreateString(global_argv[i]));
  }
  auto start_argv = builder.CreateVector(args);

  deno::MsgBuilder msg_builder(builder);
  msg_builder.add_command(deno::Command_START);
  msg_builder.add_start_cwd(start_cwd);
  msg_builder.add_start_argv(start_argv);

  auto response = msg_builder.Finish();
  builder.Finish(response);

  deno_buf bufout{reinterpret_cast<const char*>(builder.GetBufferPointer()),
                  builder.GetSize()};
  deno_set_response(d, bufout);
}

int main(int argc, char** argv) {
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
}
