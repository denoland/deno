#!/usr/bin/env python

# Copyright (c) 2012 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

#
# Xcode supports build variable substitutions and CPP; sadly, that doesn't work
# because:
#
# 1. Xcode wants to do the Info.plist work before it runs any build phases,
#    this means if we were to generate a .h file for INFOPLIST_PREFIX_HEADER
#    we'd have to put it in another target so it runs in time.
# 2. Xcode also doesn't check to see if the header being used as a prefix for
#    the Info.plist has changed.  So even if we updated it, it's only looking
#    at the modtime of the info.plist to see if that's changed.
#
# So, we work around all of this by making a script build phase that will run
# during the app build, and simply update the info.plist in place.  This way
# by the time the app target is done, the info.plist is correct.
#

import optparse
import os
import plistlib
import re
import subprocess
import sys
import tempfile

TOP = os.path.dirname(os.path.dirname(os.path.dirname(__file__)))


def _ConvertPlist(source_plist, output_plist, fmt):
  """Convert |source_plist| to |fmt| and save as |output_plist|."""
  return subprocess.call(
      ['plutil', '-convert', fmt, '-o', output_plist, source_plist])


def _GetOutput(args):
  """Runs a subprocess and waits for termination. Returns (stdout, returncode)
  of the process. stderr is attached to the parent."""
  proc = subprocess.Popen(args, stdout=subprocess.PIPE)
  (stdout, stderr) = proc.communicate()
  return (stdout, proc.returncode)


def _GetOutputNoError(args):
  """Similar to _GetOutput() but ignores stderr. If there's an error launching
  the child (like file not found), the exception will be caught and (None, 1)
  will be returned to mimic quiet failure."""
  try:
    proc = subprocess.Popen(args, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE)
  except OSError:
    return (None, 1)
  (stdout, stderr) = proc.communicate()
  return (stdout, proc.returncode)


def _RemoveKeys(plist, *keys):
  """Removes a varargs of keys from the plist."""
  for key in keys:
    try:
      del plist[key]
    except KeyError:
      pass


def _ApplyVersionOverrides(version, keys, overrides, separator='.'):
  """Applies version overrides.

  Given a |version| string as "a.b.c.d" (assuming a default separator) with
  version components named by |keys| then overrides any value that is present
  in |overrides|.

  >>> _ApplyVersionOverrides('a.b', ['major', 'minor'], {'minor': 'd'})
  'a.d'
  """
  if not overrides:
    return version
  version_values = version.split(separator)
  for i, (key, value) in enumerate(zip(keys, version_values)):
    if key in overrides:
      version_values[i] = overrides[key]
  return separator.join(version_values)


def _GetVersion(version_format, values, overrides=None):
  """Generates a version number according to |version_format| using the values
  from |values| or |overrides| if given."""
  result = version_format
  for key in values:
    if overrides and key in overrides:
      value = overrides[key]
    else:
      value = values[key]
    result = result.replace('@%s@' % key, value)
  return result


def _AddVersionKeys(
    plist, version_format_for_key, version=None, overrides=None):
  """Adds the product version number into the plist. Returns True on success and
  False on error. The error will be printed to stderr."""
  if not version:
    # Pull in the Chrome version number.
    VERSION_TOOL = os.path.join(TOP, 'build/util/version.py')
    VERSION_FILE = os.path.join(TOP, 'chrome/VERSION')
    (stdout, retval) = _GetOutput([
        VERSION_TOOL, '-f', VERSION_FILE,
        '-t', '@MAJOR@.@MINOR@.@BUILD@.@PATCH@'])

    # If the command finished with a non-zero return code, then report the
    # error up.
    if retval != 0:
      return False

    version = stdout.strip()

  # Parse the given version number, that should be in MAJOR.MINOR.BUILD.PATCH
  # format (where each value is a number). Note that str.isdigit() returns
  # True if the string is composed only of digits (and thus match \d+ regexp).
  groups = version.split('.')
  if len(groups) != 4 or not all(element.isdigit() for element in groups):
    print >>sys.stderr, 'Invalid version string specified: "%s"' % version
    return False
  values = dict(zip(('MAJOR', 'MINOR', 'BUILD', 'PATCH'), groups))

  for key in version_format_for_key:
    plist[key] = _GetVersion(version_format_for_key[key], values, overrides)

  # Return with no error.
  return True


def _DoSCMKeys(plist, add_keys):
  """Adds the SCM information, visible in about:version, to property list. If
  |add_keys| is True, it will insert the keys, otherwise it will remove them."""
  scm_revision = None
  if add_keys:
    # Pull in the Chrome revision number.
    VERSION_TOOL = os.path.join(TOP, 'build/util/version.py')
    LASTCHANGE_FILE = os.path.join(TOP, 'build/util/LASTCHANGE')
    (stdout, retval) = _GetOutput([VERSION_TOOL, '-f', LASTCHANGE_FILE, '-t',
                                  '@LASTCHANGE@'])
    if retval:
      return False
    scm_revision = stdout.rstrip()

  # See if the operation failed.
  _RemoveKeys(plist, 'SCMRevision')
  if scm_revision != None:
    plist['SCMRevision'] = scm_revision
  elif add_keys:
    print >>sys.stderr, 'Could not determine SCM revision.  This may be OK.'

  return True


def _AddBreakpadKeys(plist, branding, platform, staging):
  """Adds the Breakpad keys. This must be called AFTER _AddVersionKeys() and
  also requires the |branding| argument."""
  plist['BreakpadReportInterval'] = '3600'  # Deliberately a string.
  plist['BreakpadProduct'] = '%s_%s' % (branding, platform)
  plist['BreakpadProductDisplay'] = branding
  if staging:
    plist['BreakpadURL'] = 'https://clients2.google.com/cr/staging_report'
  else:
    plist['BreakpadURL'] = 'https://clients2.google.com/cr/report'

  # These are both deliberately strings and not boolean.
  plist['BreakpadSendAndExit'] = 'YES'
  plist['BreakpadSkipConfirm'] = 'YES'


def _RemoveBreakpadKeys(plist):
  """Removes any set Breakpad keys."""
  _RemoveKeys(plist,
      'BreakpadURL',
      'BreakpadReportInterval',
      'BreakpadProduct',
      'BreakpadProductDisplay',
      'BreakpadVersion',
      'BreakpadSendAndExit',
      'BreakpadSkipConfirm')


def _TagSuffixes():
  # Keep this list sorted in the order that tag suffix components are to
  # appear in a tag value. That is to say, it should be sorted per ASCII.
  components = ('full',)
  assert tuple(sorted(components)) == components

  components_len = len(components)
  combinations = 1 << components_len
  tag_suffixes = []
  for combination in xrange(0, combinations):
    tag_suffix = ''
    for component_index in xrange(0, components_len):
      if combination & (1 << component_index):
        tag_suffix += '-' + components[component_index]
    tag_suffixes.append(tag_suffix)
  return tag_suffixes


def _AddKeystoneKeys(plist, bundle_identifier):
  """Adds the Keystone keys. This must be called AFTER _AddVersionKeys() and
  also requires the |bundle_identifier| argument (com.example.product)."""
  plist['KSVersion'] = plist['CFBundleShortVersionString']
  plist['KSProductID'] = bundle_identifier
  plist['KSUpdateURL'] = 'https://tools.google.com/service/update2'

  _RemoveKeys(plist, 'KSChannelID')
  for tag_suffix in _TagSuffixes():
    if tag_suffix:
      plist['KSChannelID' + tag_suffix] = tag_suffix


def _RemoveKeystoneKeys(plist):
  """Removes any set Keystone keys."""
  _RemoveKeys(plist,
      'KSVersion',
      'KSProductID',
      'KSUpdateURL')

  tag_keys = []
  for tag_suffix in _TagSuffixes():
    tag_keys.append('KSChannelID' + tag_suffix)
  _RemoveKeys(plist, *tag_keys)


def Main(argv):
  parser = optparse.OptionParser('%prog [options]')
  parser.add_option('--plist', dest='plist_path', action='store',
      type='string', default=None, help='The path of the plist to tweak.')
  parser.add_option('--output', dest='plist_output', action='store',
      type='string', default=None, help='If specified, the path to output ' + \
      'the tweaked plist, rather than overwriting the input.')
  parser.add_option('--breakpad', dest='use_breakpad', action='store',
      type='int', default=False, help='Enable Breakpad [1 or 0]')
  parser.add_option('--breakpad_staging', dest='use_breakpad_staging',
      action='store_true', default=False,
      help='Use staging breakpad to upload reports. Ignored if --breakpad=0.')
  parser.add_option('--keystone', dest='use_keystone', action='store',
      type='int', default=False, help='Enable Keystone [1 or 0]')
  parser.add_option('--scm', dest='add_scm_info', action='store', type='int',
      default=True, help='Add SCM metadata [1 or 0]')
  parser.add_option('--branding', dest='branding', action='store',
      type='string', default=None, help='The branding of the binary')
  parser.add_option('--bundle_id', dest='bundle_identifier',
      action='store', type='string', default=None,
      help='The bundle id of the binary')
  parser.add_option('--platform', choices=('ios', 'mac'), default='mac',
      help='The target platform of the bundle')
  parser.add_option('--version-overrides', action='append',
      help='Key-value pair to override specific component of version '
           'like key=value (can be passed multiple time to configure '
           'more than one override)')
  parser.add_option('--format', choices=('binary1', 'xml1', 'json'),
      default='xml1', help='Format to use when writing property list '
          '(default: %(default)s)')
  parser.add_option('--version', dest='version', action='store', type='string',
      default=None, help='The version string [major.minor.build.patch]')
  (options, args) = parser.parse_args(argv)

  if len(args) > 0:
    print >>sys.stderr, parser.get_usage()
    return 1

  if not options.plist_path:
    print >>sys.stderr, 'No --plist specified.'
    return 1

  # Read the plist into its parsed format. Convert the file to 'xml1' as
  # plistlib only supports that format in Python 2.7.
  with tempfile.NamedTemporaryFile() as temp_info_plist:
    retcode = _ConvertPlist(options.plist_path, temp_info_plist.name, 'xml1')
    if retcode != 0:
      return retcode
    plist = plistlib.readPlist(temp_info_plist.name)

  # Convert overrides.
  overrides = {}
  if options.version_overrides:
    for pair in options.version_overrides:
      if not '=' in pair:
        print >>sys.stderr, 'Invalid value for --version-overrides:', pair
        return 1
      key, value = pair.split('=', 1)
      overrides[key] = value
      if key not in ('MAJOR', 'MINOR', 'BUILD', 'PATCH'):
        print >>sys.stderr, 'Unsupported key for --version-overrides:', key
        return 1

  if options.platform == 'mac':
    version_format_for_key = {
      # Add public version info so "Get Info" works.
      'CFBundleShortVersionString': '@MAJOR@.@MINOR@.@BUILD@.@PATCH@',

      # Honor the 429496.72.95 limit.  The maximum comes from splitting 2^32 - 1
      # into  6, 2, 2 digits.  The limitation was present in Tiger, but it could
      # have been fixed in later OS release, but hasn't been tested (it's easy
      # enough to find out with "lsregister -dump).
      # http://lists.apple.com/archives/carbon-dev/2006/Jun/msg00139.html
      # BUILD will always be an increasing value, so BUILD_PATH gives us
      # something unique that meetings what LS wants.
      'CFBundleVersion': '@BUILD@.@PATCH@',
    }
  else:
    version_format_for_key = {
      'CFBundleShortVersionString': '@MAJOR@.@BUILD@.@PATCH@',
      'CFBundleVersion': '@MAJOR@.@MINOR@.@BUILD@.@PATCH@'
    }

  if options.use_breakpad:
    version_format_for_key['BreakpadVersion'] = \
        '@MAJOR@.@MINOR@.@BUILD@.@PATCH@'

  # Insert the product version.
  if not _AddVersionKeys(
      plist, version_format_for_key, version=options.version,
      overrides=overrides):
    return 2

  # Add Breakpad if configured to do so.
  if options.use_breakpad:
    if options.branding is None:
      print >>sys.stderr, 'Use of Breakpad requires branding.'
      return 1
    # Map "target_os" passed from gn via the --platform parameter
    # to the platform as known by breakpad.
    platform = {'mac': 'Mac', 'ios': 'iOS'}[options.platform]
    _AddBreakpadKeys(plist, options.branding, platform,
        options.use_breakpad_staging)
  else:
    _RemoveBreakpadKeys(plist)

  # Add Keystone if configured to do so.
  if options.use_keystone:
    if options.bundle_identifier is None:
      print >>sys.stderr, 'Use of Keystone requires the bundle id.'
      return 1
    _AddKeystoneKeys(plist, options.bundle_identifier)
  else:
    _RemoveKeystoneKeys(plist)

  # Adds or removes any SCM keys.
  if not _DoSCMKeys(plist, options.add_scm_info):
    return 3

  output_path = options.plist_path
  if options.plist_output is not None:
    output_path = options.plist_output

  # Now that all keys have been mutated, rewrite the file.
  with tempfile.NamedTemporaryFile() as temp_info_plist:
    plistlib.writePlist(plist, temp_info_plist.name)

    # Convert Info.plist to the format requested by the --format flag. Any
    # format would work on Mac but iOS requires specific format.
    return _ConvertPlist(temp_info_plist.name, output_path, options.format)


if __name__ == '__main__':
  sys.exit(Main(sys.argv[1:]))
