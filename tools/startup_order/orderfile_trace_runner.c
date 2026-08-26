// Copyright 2018-2026 the Deno authors. MIT license.
// Run one traced workload while the Deno-based generator is stopped.
//
// The generator's own V8 threads otherwise perturb concurrent first-touch
// ordering in the child. A pipe keeps the child before exec until SIGSTOP has
// been sent to the generator. The generator resumes when the child exits.

#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

static pid_t child_pid;
static pid_t generator_pid;

static void on_alarm(int signal_number) {
  (void)signal_number;
  if (child_pid > 0) {
    kill(child_pid, SIGKILL);
  }
}

int main(int argc, char **argv) {
  if (argc < 3) {
    fprintf(stderr, "usage: %s GENERATOR_PID COMMAND [ARGS...]\n", argv[0]);
    return 2;
  }

  char *end;
  long parsed_pid = strtol(argv[1], &end, 10);
  if (*argv[1] == '\0' || *end != '\0' || parsed_pid <= 0) {
    fprintf(stderr, "invalid generator pid: %s\n", argv[1]);
    return 2;
  }
  generator_pid = (pid_t)parsed_pid;

  int gate[2];
  if (pipe(gate) != 0) {
    perror("pipe");
    return 1;
  }

  child_pid = fork();
  if (child_pid < 0) {
    perror("fork");
    return 1;
  }
  if (child_pid == 0) {
    close(gate[1]);
    const char *preload = getenv("DENO_ORDER_RUNNER_PRELOAD");
    if (preload == NULL) {
      fprintf(stderr, "missing DENO_ORDER_RUNNER_PRELOAD\n");
      _exit(127);
    }
#if defined(__APPLE__)
    if (setenv("DYLD_INSERT_LIBRARIES", preload, 1) != 0) {
#else
    if (setenv("LD_PRELOAD", preload, 1) != 0) {
#endif
      perror("setting tracer preload");
      _exit(127);
    }
    unsetenv("DENO_ORDER_RUNNER_PRELOAD");
    char byte;
    while (read(gate[0], &byte, 1) < 0 && errno == EINTR) {
    }
    close(gate[0]);
    execvp(argv[2], &argv[2]);
    perror("execvp");
    _exit(127);
  }

  close(gate[0]);
  if (kill(generator_pid, SIGSTOP) != 0) {
    perror("stopping generator");
    kill(child_pid, SIGKILL);
  }
  char byte = 0;
  if (write(gate[1], &byte, 1) != 1) {
    perror("releasing child");
    kill(child_pid, SIGKILL);
  }
  close(gate[1]);

  signal(SIGALRM, on_alarm);
  alarm(120);
  int status;
  while (waitpid(child_pid, &status, 0) < 0) {
    if (errno != EINTR) {
      perror("waitpid");
      status = 1 << 8;
      break;
    }
  }
  alarm(0);
  kill(generator_pid, SIGCONT);

  if (WIFEXITED(status)) {
    return WEXITSTATUS(status);
  }
  if (WIFSIGNALED(status)) {
    return 128 + WTERMSIG(status);
  }
  return 1;
}
