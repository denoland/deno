// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <string>

#include "v8/src/base/logging.h"
#include "./msg.pb.h"
#include "include/deno.h"

void MessagesFromJS(Deno* d, const char* channel, deno_buf buf) {
  printf("MessagesFromJS %s\n", channel);

  char cwdbuf[1024];
  std::string cwd(getcwd(cwdbuf, sizeof(cwdbuf)));

  deno::Msg response;
  response.set_command(deno::Msg_Command_START);
  response.set_start_cwd(cwd);

  std::string output;
  CHECK(response.SerializeToString(&output));

  auto bufout = deno_buf{output.c_str(), output.length()};
  deno_set_response(d, bufout);
}

int main(int argc, char** argv) {
  deno_init();

  Deno* d = deno_new(NULL, MessagesFromJS);
  bool r = deno_execute(d, "deno_main.js", "denoMain();");
  if (!r) {
    printf("Error! %s\n", deno_last_exception(d));
    exit(1);
  }
  deno_delete(d);
}
