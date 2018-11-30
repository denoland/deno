# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import contextlib
import logging
import os

from pylib.constants import host_paths

_COLORAMA_PATH = os.path.join(
    host_paths.DIR_SOURCE_ROOT, 'third_party', 'colorama', 'src')

with host_paths.SysPath(_COLORAMA_PATH, position=0):
  import colorama

BACK = colorama.Back
FORE = colorama.Fore
STYLE = colorama.Style


class _ColorFormatter(logging.Formatter):
  # pylint does not see members added dynamically in the constructor.
  # pylint: disable=no-member
  color_map = {
    logging.DEBUG: (FORE.CYAN),
    logging.WARNING: (FORE.YELLOW),
    logging.ERROR: (FORE.RED),
    logging.CRITICAL: (BACK.RED),
  }

  def __init__(self, wrapped_formatter=None):
    """Wraps a |logging.Formatter| and adds color."""
    super(_ColorFormatter, self).__init__(self)
    self._wrapped_formatter = wrapped_formatter or logging.Formatter()

  #override
  def format(self, record):
    message = self._wrapped_formatter.format(record)
    return self.Colorize(message, record.levelno)

  def Colorize(self, message, log_level):
    try:
      return (''.join(self.color_map[log_level]) + message +
              colorama.Style.RESET_ALL)
    except KeyError:
      return message


class ColorStreamHandler(logging.StreamHandler):
  """Handler that can be used to colorize logging output.

  Example using a specific logger:

    logger = logging.getLogger('my_logger')
    logger.addHandler(ColorStreamHandler())
    logger.info('message')

  Example using the root logger:

    ColorStreamHandler.MakeDefault()
    logging.info('message')

  """
  def __init__(self, force_color=False):
    super(ColorStreamHandler, self).__init__()
    self.force_color = force_color
    self.setFormatter(logging.Formatter())

  @property
  def is_tty(self):
    isatty = getattr(self.stream, 'isatty', None)
    return isatty and isatty()

  #override
  def setFormatter(self, formatter):
    if self.force_color or self.is_tty:
      formatter = _ColorFormatter(formatter)
    super(ColorStreamHandler, self).setFormatter(formatter)

  @staticmethod
  def MakeDefault(force_color=False):
     """
     Replaces the default logging handlers with a coloring handler. To use
     a colorizing handler at the same time as others, either register them
     after this call, or add the ColorStreamHandler on the logger using
     Logger.addHandler()

     Args:
       force_color: Set to True to bypass the tty check and always colorize.
     """
     # If the existing handlers aren't removed, messages are duplicated
     logging.getLogger().handlers = []
     logging.getLogger().addHandler(ColorStreamHandler(force_color))


@contextlib.contextmanager
def OverrideColor(level, color):
  """Temporarily override the logging color for a specified level.

  Args:
    level: logging level whose color gets overridden.
    color: tuple of formats to apply to log lines.
  """
  prev_colors = {}
  for handler in logging.getLogger().handlers:
    if isinstance(handler.formatter, _ColorFormatter):
      prev_colors[handler.formatter] = handler.formatter.color_map[level]
      handler.formatter.color_map[level] = color
  try:
    yield
  finally:
    for formatter, prev_color in prev_colors.iteritems():
      formatter.color_map[level] = prev_color


@contextlib.contextmanager
def SuppressLogging(level=logging.ERROR):
  """Momentarilly suppress logging events from all loggers.

  TODO(jbudorick): This is not thread safe. Log events from other threads might
  also inadvertently disappear.

  Example:

    with logging_utils.SuppressLogging():
      # all but CRITICAL logging messages are suppressed
      logging.info('just doing some thing') # not shown
      logging.critical('something really bad happened') # still shown

  Args:
    level: logging events with this or lower levels are suppressed.
  """
  logging.disable(level)
  yield
  logging.disable(logging.NOTSET)
