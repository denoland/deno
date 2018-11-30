# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Functions to instrument all Python function calls.

This generates a JSON file readable by Chrome's about:tracing. To use it,
either call start_instrumenting and stop_instrumenting at the appropriate times,
or use the Instrument context manager.

A function is only traced if it is from a Python module that matches at least
one regular expression object in to_include, and does not match any in
to_exclude. In between the start and stop events, every function call of a
function from such a module will be added to the trace.
"""

import contextlib
import functools
import inspect
import os
import re
import sys
import threading

from py_trace_event import trace_event


# Modules to exclude by default (to avoid problems like infinite loops)
DEFAULT_EXCLUDE = [r'py_trace_event\..*']

class _TraceArguments(object):
  def __init__(self):
    """Wraps a dictionary to ensure safe evaluation of repr()."""
    self._arguments = {}

  @staticmethod
  def _safeStringify(item):
    try:
      item_str = repr(item)
    except Exception: # pylint: disable=broad-except
      try:
        item_str = str(item)
      except Exception: # pylint: disable=broad-except
        item_str = "<ERROR>"
    return item_str

  def add(self, key, val):
    key_str = _TraceArguments._safeStringify(key)
    val_str = _TraceArguments._safeStringify(val)

    self._arguments[key_str] = val_str

  def __repr__(self):
    return repr(self._arguments)


saved_thread_ids = set()

def _shouldTrace(frame, to_include, to_exclude, included, excluded):
  """
  Decides whether or not the function called in frame should be traced.

  Args:
    frame: The Python frame object of this function call.
    to_include: Set of regex objects for modules which should be traced.
    to_exclude: Set of regex objects for modules which should not be traced.
    included: Set of module names we've determined should be traced.
    excluded: Set of module names we've determined should not be traced.
  """
  if not inspect.getmodule(frame):
    return False

  module_name = inspect.getmodule(frame).__name__

  if module_name in included:
    includes = True
  elif to_include:
    includes = any([pattern.match(module_name) for pattern in to_include])
  else:
    includes = True

  if includes:
    included.add(module_name)
  else:
    return False

  # Find the modules of every function in the stack trace.
  frames = inspect.getouterframes(frame)
  calling_module_names = [inspect.getmodule(fr[0]).__name__ for fr in frames]

  # Return False for anything with an excluded module's function anywhere in the
  # stack trace (even if the function itself is in an included module).
  if to_exclude:
    for calling_module in calling_module_names:
      if calling_module in excluded:
        return False
      for pattern in to_exclude:
        if pattern.match(calling_module):
          excluded.add(calling_module)
          return False

  return True

def _generate_trace_function(to_include, to_exclude):
  to_include = {re.compile(item) for item in to_include}
  to_exclude = {re.compile(item) for item in to_exclude}
  to_exclude.update({re.compile(item) for item in DEFAULT_EXCLUDE})

  included = set()
  excluded = set()

  tracing_pid = os.getpid()

  def traceFunction(frame, event, arg):
    del arg

    # Don't try to trace in subprocesses.
    if os.getpid() != tracing_pid:
      sys.settrace(None)
      return None

    # pylint: disable=unused-argument
    if event not in ("call", "return"):
      return None

    function_name = frame.f_code.co_name
    filename = frame.f_code.co_filename
    line_number = frame.f_lineno

    if _shouldTrace(frame, to_include, to_exclude, included, excluded):
      if event == "call":
        # This function is beginning; we save the thread name (if that hasn't
        # been done), record the Begin event, and return this function to be
        # used as the local trace function.

        thread_id = threading.current_thread().ident

        if thread_id not in saved_thread_ids:
          thread_name = threading.current_thread().name

          trace_event.trace_set_thread_name(thread_name)

          saved_thread_ids.add(thread_id)

        arguments = _TraceArguments()
        # The function's argument values are stored in the frame's
        # |co_varnames| as the first |co_argcount| elements. (Following that
        # are local variables.)
        for idx in range(frame.f_code.co_argcount):
          arg_name = frame.f_code.co_varnames[idx]
          arguments.add(arg_name, frame.f_locals[arg_name])
        trace_event.trace_begin(function_name, arguments=arguments,
                                module=inspect.getmodule(frame).__name__,
                                filename=filename, line_number=line_number)

        # Return this function, so it gets used as the "local trace function"
        # within this function's frame (and in particular, gets called for this
        # function's "return" event).
        return traceFunction

      if event == "return":
        trace_event.trace_end(function_name)
        return None

  return traceFunction


def no_tracing(f):
  @functools.wraps(f)
  def wrapper(*args, **kwargs):
    trace_func = sys.gettrace()
    try:
      sys.settrace(None)
      threading.settrace(None)
      return f(*args, **kwargs)
    finally:
      sys.settrace(trace_func)
      threading.settrace(trace_func)
  return wrapper


def start_instrumenting(output_file, to_include=(), to_exclude=()):
  """Enable tracing of all function calls (from specified modules)."""
  trace_event.trace_enable(output_file)

  traceFunc = _generate_trace_function(to_include, to_exclude)
  sys.settrace(traceFunc)
  threading.settrace(traceFunc)


def stop_instrumenting():
  trace_event.trace_disable()

  sys.settrace(None)
  threading.settrace(None)


@contextlib.contextmanager
def Instrument(output_file, to_include=(), to_exclude=()):
  try:
    start_instrumenting(output_file, to_include, to_exclude)
    yield None
  finally:
    stop_instrumenting()
