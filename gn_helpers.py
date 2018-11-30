# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Helper functions useful when writing scripts that integrate with GN.

The main functions are ToGNString and FromGNString which convert between
serialized GN veriables and Python variables.

To use in a random python file in the build:

  import os
  import sys

  sys.path.append(os.path.join(os.path.dirname(__file__),
                               os.pardir, os.pardir, "build"))
  import gn_helpers

Where the sequence of parameters to join is the relative path from your source
file to the build directory."""

class GNException(Exception):
  pass


def ToGNString(value, allow_dicts = True):
  """Returns a stringified GN equivalent of the Python value.

  allow_dicts indicates if this function will allow converting dictionaries
  to GN scopes. This is only possible at the top level, you can't nest a
  GN scope in a list, so this should be set to False for recursive calls."""
  if isinstance(value, basestring):
    if value.find('\n') >= 0:
      raise GNException("Trying to print a string with a newline in it.")
    return '"' + \
        value.replace('\\', '\\\\').replace('"', '\\"').replace('$', '\\$') + \
        '"'

  if isinstance(value, unicode):
    return ToGNString(value.encode('utf-8'))

  if isinstance(value, bool):
    if value:
      return "true"
    return "false"

  if isinstance(value, list):
    return '[ %s ]' % ', '.join(ToGNString(v) for v in value)

  if isinstance(value, dict):
    if not allow_dicts:
      raise GNException("Attempting to recursively print a dictionary.")
    result = ""
    for key in sorted(value):
      if not isinstance(key, basestring):
        raise GNException("Dictionary key is not a string.")
      result += "%s = %s\n" % (key, ToGNString(value[key], False))
    return result

  if isinstance(value, int):
    return str(value)

  raise GNException("Unsupported type when printing to GN.")


def FromGNString(input_string):
  """Converts the input string from a GN serialized value to Python values.

  For details on supported types see GNValueParser.Parse() below.

  If your GN script did:
    something = [ "file1", "file2" ]
    args = [ "--values=$something" ]
  The command line would look something like:
    --values="[ \"file1\", \"file2\" ]"
  Which when interpreted as a command line gives the value:
    [ "file1", "file2" ]

  You can parse this into a Python list using GN rules with:
    input_values = FromGNValues(options.values)
  Although the Python 'ast' module will parse many forms of such input, it
  will not handle GN escaping properly, nor GN booleans. You should use this
  function instead.


  A NOTE ON STRING HANDLING:

  If you just pass a string on the command line to your Python script, or use
  string interpolation on a string variable, the strings will not be quoted:
    str = "asdf"
    args = [ str, "--value=$str" ]
  Will yield the command line:
    asdf --value=asdf
  The unquoted asdf string will not be valid input to this function, which
  accepts only quoted strings like GN scripts. In such cases, you can just use
  the Python string literal directly.

  The main use cases for this is for other types, in particular lists. When
  using string interpolation on a list (as in the top example) the embedded
  strings will be quoted and escaped according to GN rules so the list can be
  re-parsed to get the same result."""
  parser = GNValueParser(input_string)
  return parser.Parse()


def FromGNArgs(input_string):
  """Converts a string with a bunch of gn arg assignments into a Python dict.

  Given a whitespace-separated list of

    <ident> = (integer | string | boolean | <list of the former>)

  gn assignments, this returns a Python dict, i.e.:

    FromGNArgs("foo=true\nbar=1\n") -> { 'foo': True, 'bar': 1 }.

  Only simple types and lists supported; variables, structs, calls
  and other, more complicated things are not.

  This routine is meant to handle only the simple sorts of values that
  arise in parsing --args.
  """
  parser = GNValueParser(input_string)
  return parser.ParseArgs()


def UnescapeGNString(value):
  """Given a string with GN escaping, returns the unescaped string.

  Be careful not to feed with input from a Python parsing function like
  'ast' because it will do Python unescaping, which will be incorrect when
  fed into the GN unescaper."""
  result = ''
  i = 0
  while i < len(value):
    if value[i] == '\\':
      if i < len(value) - 1:
        next_char = value[i + 1]
        if next_char in ('$', '"', '\\'):
          # These are the escaped characters GN supports.
          result += next_char
          i += 1
        else:
          # Any other backslash is a literal.
          result += '\\'
    else:
      result += value[i]
    i += 1
  return result


def _IsDigitOrMinus(char):
  return char in "-0123456789"


class GNValueParser(object):
  """Duplicates GN parsing of values and converts to Python types.

  Normally you would use the wrapper function FromGNValue() below.

  If you expect input as a specific type, you can also call one of the Parse*
  functions directly. All functions throw GNException on invalid input. """
  def __init__(self, string):
    self.input = string
    self.cur = 0

  def IsDone(self):
    return self.cur == len(self.input)

  def ConsumeWhitespace(self):
    while not self.IsDone() and self.input[self.cur] in ' \t\n':
      self.cur += 1

  def Parse(self):
    """Converts a string representing a printed GN value to the Python type.

    See additional usage notes on FromGNString above.

    - GN booleans ('true', 'false') will be converted to Python booleans.

    - GN numbers ('123') will be converted to Python numbers.

    - GN strings (double-quoted as in '"asdf"') will be converted to Python
      strings with GN escaping rules. GN string interpolation (embedded
      variables preceded by $) are not supported and will be returned as
      literals.

    - GN lists ('[1, "asdf", 3]') will be converted to Python lists.

    - GN scopes ('{ ... }') are not supported."""
    result = self._ParseAllowTrailing()
    self.ConsumeWhitespace()
    if not self.IsDone():
      raise GNException("Trailing input after parsing:\n  " +
                        self.input[self.cur:])
    return result

  def ParseArgs(self):
    """Converts a whitespace-separated list of ident=literals to a dict.

    See additional usage notes on FromGNArgs, above.
    """
    d = {}

    self.ConsumeWhitespace()
    while not self.IsDone():
      ident = self._ParseIdent()
      self.ConsumeWhitespace()
      if self.input[self.cur] != '=':
        raise GNException("Unexpected token: " + self.input[self.cur:])
      self.cur += 1
      self.ConsumeWhitespace()
      val = self._ParseAllowTrailing()
      self.ConsumeWhitespace()
      d[ident] = val

    return d

  def _ParseAllowTrailing(self):
    """Internal version of Parse that doesn't check for trailing stuff."""
    self.ConsumeWhitespace()
    if self.IsDone():
      raise GNException("Expected input to parse.")

    next_char = self.input[self.cur]
    if next_char == '[':
      return self.ParseList()
    elif _IsDigitOrMinus(next_char):
      return self.ParseNumber()
    elif next_char == '"':
      return self.ParseString()
    elif self._ConstantFollows('true'):
      return True
    elif self._ConstantFollows('false'):
      return False
    else:
      raise GNException("Unexpected token: " + self.input[self.cur:])

  def _ParseIdent(self):
    ident = ''

    next_char = self.input[self.cur]
    if not next_char.isalpha() and not next_char=='_':
      raise GNException("Expected an identifier: " + self.input[self.cur:])

    ident += next_char
    self.cur += 1

    next_char = self.input[self.cur]
    while next_char.isalpha() or next_char.isdigit() or next_char=='_':
      ident += next_char
      self.cur += 1
      next_char = self.input[self.cur]

    return ident

  def ParseNumber(self):
    self.ConsumeWhitespace()
    if self.IsDone():
      raise GNException('Expected number but got nothing.')

    begin = self.cur

    # The first character can include a negative sign.
    if not self.IsDone() and _IsDigitOrMinus(self.input[self.cur]):
      self.cur += 1
    while not self.IsDone() and self.input[self.cur].isdigit():
      self.cur += 1

    number_string = self.input[begin:self.cur]
    if not len(number_string) or number_string == '-':
      raise GNException("Not a valid number.")
    return int(number_string)

  def ParseString(self):
    self.ConsumeWhitespace()
    if self.IsDone():
      raise GNException('Expected string but got nothing.')

    if self.input[self.cur] != '"':
      raise GNException('Expected string beginning in a " but got:\n  ' +
                        self.input[self.cur:])
    self.cur += 1  # Skip over quote.

    begin = self.cur
    while not self.IsDone() and self.input[self.cur] != '"':
      if self.input[self.cur] == '\\':
        self.cur += 1  # Skip over the backslash.
        if self.IsDone():
          raise GNException("String ends in a backslash in:\n  " +
                            self.input)
      self.cur += 1

    if self.IsDone():
      raise GNException('Unterminated string:\n  ' + self.input[begin:])

    end = self.cur
    self.cur += 1  # Consume trailing ".

    return UnescapeGNString(self.input[begin:end])

  def ParseList(self):
    self.ConsumeWhitespace()
    if self.IsDone():
      raise GNException('Expected list but got nothing.')

    # Skip over opening '['.
    if self.input[self.cur] != '[':
      raise GNException("Expected [ for list but got:\n  " +
                        self.input[self.cur:])
    self.cur += 1
    self.ConsumeWhitespace()
    if self.IsDone():
      raise GNException("Unterminated list:\n  " + self.input)

    list_result = []
    previous_had_trailing_comma = True
    while not self.IsDone():
      if self.input[self.cur] == ']':
        self.cur += 1  # Skip over ']'.
        return list_result

      if not previous_had_trailing_comma:
        raise GNException("List items not separated by comma.")

      list_result += [ self._ParseAllowTrailing() ]
      self.ConsumeWhitespace()
      if self.IsDone():
        break

      # Consume comma if there is one.
      previous_had_trailing_comma = self.input[self.cur] == ','
      if previous_had_trailing_comma:
        # Consume comma.
        self.cur += 1
        self.ConsumeWhitespace()

    raise GNException("Unterminated list:\n  " + self.input)

  def _ConstantFollows(self, constant):
    """Returns true if the given constant follows immediately at the current
    location in the input. If it does, the text is consumed and the function
    returns true. Otherwise, returns false and the current position is
    unchanged."""
    end = self.cur + len(constant)
    if end > len(self.input):
      return False  # Not enough room.
    if self.input[self.cur:end] == constant:
      self.cur = end
      return True
    return False
