# Copyright 2015 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse


class CustomHelpAction(argparse.Action):
  '''Allows defining custom help actions.

  Help actions can run even when the parser would otherwise fail on missing
  arguments. The first help or custom help command mentioned on the command
  line will have its help text displayed.

  Usage:
      parser = argparse.ArgumentParser(...)
      CustomHelpAction.EnableFor(parser)
      parser.add_argument('--foo-help',
                          action='custom_help',
                          custom_help_text='this is the help message',
                          help='What this helps with')
  '''
  # Derived from argparse._HelpAction from
  # https://github.com/python/cpython/blob/master/Lib/argparse.py

  # pylint: disable=redefined-builtin
  # (complains about 'help' being redefined)
  def __init__(self,
               option_strings,
               dest=argparse.SUPPRESS,
               default=argparse.SUPPRESS,
               custom_help_text=None,
               help=None):
    super(CustomHelpAction, self).__init__(option_strings=option_strings,
                                           dest=dest,
                                           default=default,
                                           nargs=0,
                                           help=help)

    if not custom_help_text:
      raise ValueError('custom_help_text is required')
    self._help_text = custom_help_text

  def __call__(self, parser, namespace, values, option_string=None):
    print self._help_text
    parser.exit()

  @staticmethod
  def EnableFor(parser):
    parser.register('action', 'custom_help', CustomHelpAction)
