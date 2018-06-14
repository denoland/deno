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

#include "./msg.pb.h"
#include "include/deno.h"
#include "v8/src/base/logging.h"

static char** global_argv;
static int global_argc;

void MessagesFromJS(Deno* d, const char* channel, deno_buf buf) {
  printf("MessagesFromJS %s\n", channel);

  deno::Msg response;
  response.set_command(deno::Msg_Command_START);

  char cwdbuf[1024];
  // TODO(piscisaureus): support unicode on windows.
  std::string cwd(getcwd(cwdbuf, sizeof(cwdbuf)));
  response.set_start_cwd(cwd);

  for (int i = 0; i < global_argc; ++i) {
    printf("arg %d %s\n", i, global_argv[i]);
    response.add_start_argv(global_argv[i]);
  }
  printf("response.start_argv_size %d \n", response.start_argv_size());

  std::string output;
  CHECK(response.SerializeToString(&output));

  deno_buf bufout{output.c_str(), output.length()};
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
    printf("Error! %s\n", deno_last_exception(d));
    exit(1);
  }
  deno_delete(d);
}
