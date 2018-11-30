#!/usr/bin/env python
#
# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Renders one or more template files using the Jinja template engine."""

import codecs
import argparse
import os
import sys

from util import build_utils
from util import resource_utils

sys.path.append(os.path.join(os.path.dirname(__file__), os.pardir))
from pylib.constants import host_paths

# Import jinja2 from third_party/jinja2
sys.path.append(os.path.join(host_paths.DIR_SOURCE_ROOT, 'third_party'))
import jinja2  # pylint: disable=F0401


class _RecordingFileSystemLoader(jinja2.FileSystemLoader):
  def __init__(self, searchpath):
    jinja2.FileSystemLoader.__init__(self, searchpath)
    self.loaded_templates = set()

  def get_source(self, environment, template):
    contents, filename, uptodate = jinja2.FileSystemLoader.get_source(
        self, environment, template)
    self.loaded_templates.add(os.path.relpath(filename))
    return contents, filename, uptodate


class JinjaProcessor(object):
  """Allows easy rendering of jinja templates with input file tracking."""
  def __init__(self, loader_base_dir, variables=None):
    self.loader_base_dir = loader_base_dir
    self.variables = variables or {}
    self.loader = _RecordingFileSystemLoader(loader_base_dir)
    self.env = jinja2.Environment(loader=self.loader)
    self.env.undefined = jinja2.StrictUndefined
    self.env.line_comment_prefix = '##'
    self.env.trim_blocks = True
    self.env.lstrip_blocks = True
    self._template_cache = {}  # Map of path -> Template

  def Render(self, input_filename, variables=None):
    input_rel_path = os.path.relpath(input_filename, self.loader_base_dir)
    template = self._template_cache.get(input_rel_path)
    if not template:
      template = self.env.get_template(input_rel_path)
      self._template_cache[input_rel_path] = template
    return template.render(variables or self.variables)

  def GetLoadedTemplates(self):
    return list(self.loader.loaded_templates)


def _ProcessFile(processor, input_filename, output_filename):
  output = processor.Render(input_filename)

  # If |output| is same with the file content, we skip update and
  # ninja's restat will avoid rebuilding things that depend on it.
  if os.path.isfile(output_filename):
    with codecs.open(output_filename, 'r', 'utf-8') as f:
      if f.read() == output:
        return

  with codecs.open(output_filename, 'w', 'utf-8') as output_file:
    output_file.write(output)


def _ProcessFiles(processor, input_filenames, inputs_base_dir, outputs_zip):
  with build_utils.TempDir() as temp_dir:
    files_to_zip = dict()
    for input_filename in input_filenames:
      relpath = os.path.relpath(os.path.abspath(input_filename),
                                os.path.abspath(inputs_base_dir))
      if relpath.startswith(os.pardir):
        raise Exception('input file %s is not contained in inputs base dir %s'
                        % (input_filename, inputs_base_dir))

      output_filename = os.path.join(temp_dir, relpath)
      parent_dir = os.path.dirname(output_filename)
      build_utils.MakeDirectory(parent_dir)
      _ProcessFile(processor, input_filename, output_filename)
      files_to_zip[relpath] = input_filename

    resource_utils.CreateResourceInfoFile(files_to_zip, outputs_zip)
    build_utils.ZipDir(outputs_zip, temp_dir)


def _ParseVariables(variables_arg, error_func):
  variables = {}
  for v in build_utils.ParseGnList(variables_arg):
    if '=' not in v:
      error_func('--variables argument must contain "=": ' + v)
    name, _, value = v.partition('=')
    variables[name] = value
  return variables


def main():
  parser = argparse.ArgumentParser()
  parser.add_argument('--inputs', required=True,
                      help='GN-list of template files to process.')
  parser.add_argument('--includes', default='',
                      help="GN-list of files that get {% include %}'ed.")
  parser.add_argument('--output', help='The output file to generate. Valid '
                      'only if there is a single input.')
  parser.add_argument('--outputs-zip', help='A zip file for the processed '
                      'templates. Required if there are multiple inputs.')
  parser.add_argument('--inputs-base-dir', help='A common ancestor directory '
                      'of the inputs. Each output\'s path in the output zip '
                      'will match the relative path from INPUTS_BASE_DIR to '
                      'the input. Required if --output-zip is given.')
  parser.add_argument('--loader-base-dir', help='Base path used by the '
                      'template loader. Must be a common ancestor directory of '
                      'the inputs. Defaults to DIR_SOURCE_ROOT.',
                      default=host_paths.DIR_SOURCE_ROOT)
  parser.add_argument('--variables', help='Variables to be made available in '
                      'the template processing environment, as a GYP list '
                      '(e.g. --variables "channel=beta mstone=39")', default='')
  parser.add_argument('--check-includes', action='store_true',
                      help='Enable inputs and includes checks.')
  options = parser.parse_args()

  inputs = build_utils.ParseGnList(options.inputs)
  includes = build_utils.ParseGnList(options.includes)

  if (options.output is None) == (options.outputs_zip is None):
    parser.error('Exactly one of --output and --output-zip must be given')
  if options.output and len(inputs) != 1:
    parser.error('--output cannot be used with multiple inputs')
  if options.outputs_zip and not options.inputs_base_dir:
    parser.error('--inputs-base-dir must be given when --output-zip is used')

  variables = _ParseVariables(options.variables, parser.error)
  processor = JinjaProcessor(options.loader_base_dir, variables=variables)

  if options.output:
    _ProcessFile(processor, inputs[0], options.output)
  else:
    _ProcessFiles(processor, inputs, options.inputs_base_dir,
                  options.outputs_zip)

  if options.check_includes:
    all_inputs = set(processor.GetLoadedTemplates())
    all_inputs.difference_update(inputs)
    all_inputs.difference_update(includes)
    if all_inputs:
      raise Exception('Found files not listed via --includes:\n' +
                      '\n'.join(sorted(all_inputs)))


if __name__ == '__main__':
  main()
