# Copyright 2016 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import os
import shutil
import tempfile
from xml.etree import ElementTree

from devil.utils import cmd_helper
from pylib import constants

DEXDUMP_PATH = os.path.join(constants.ANDROID_SDK_TOOLS, 'dexdump')


def Dump(apk_path):
  """Dumps class and method information from a APK into a dict via dexdump.

  Args:
    apk_path: An absolute path to an APK file to dump.
  Returns:
    A dict in the following format:
      {
        <package_name>: {
          'classes': {
            <class_name>: {
              'methods': [<method_1>, <method_2>]
            }
          }
        }
      }
  """
  # TODO(mikecase): Support multi-dex
  try:
    dexfile_dir = tempfile.mkdtemp()
    # Python zipfile module is unable to unzip APKs.
    cmd_helper.RunCmd(['unzip', apk_path, 'classes.dex'], cwd=dexfile_dir)
    dexfile = os.path.join(dexfile_dir, 'classes.dex')
    output_xml = cmd_helper.GetCmdOutput([DEXDUMP_PATH, '-l', 'xml', dexfile])
    return _ParseRootNode(ElementTree.fromstring(output_xml))
  finally:
    shutil.rmtree(dexfile_dir)


def _ParseRootNode(root):
  """Parses the XML output of dexdump. This output is in the following format.

  This is a subset of the information contained within dexdump output.

  <api>
    <package name="foo.bar">
      <class name="Class" extends="foo.bar.SuperClass">
        <field name="Field">
        </field>
        <constructor name="Method">
          <parameter name="Param" type="int">
          </parameter>
        </constructor>
        <method name="Method">
          <parameter name="Param" type="int">
          </parameter>
        </method>
      </class>
    </package>
  </api>
  """
  results = {}
  for child in root:
    if child.tag == 'package':
      package_name = child.attrib['name']
      parsed_node = _ParsePackageNode(child)
      if package_name in results:
        results[package_name]['classes'].update(parsed_node['classes'])
      else:
        results[package_name] = parsed_node
  return results


def _ParsePackageNode(package_node):
  """Parses a <package> node from the dexdump xml output.

  Returns:
    A dict in the format:
      {
        'classes': {
          <class_1>: {
            'methods': [<method_1>, <method_2>]
          },
          <class_2>: {
            'methods': [<method_1>, <method_2>]
          },
        }
      }
  """
  classes = {}
  for child in package_node:
    if child.tag == 'class':
      classes[child.attrib['name']] = _ParseClassNode(child)
  return {'classes': classes}


def _ParseClassNode(class_node):
  """Parses a <class> node from the dexdump xml output.

  Returns:
    A dict in the format:
      {
        'methods': [<method_1>, <method_2>]
      }
  """
  methods = []
  for child in class_node:
    if child.tag == 'method':
      methods.append(child.attrib['name'])
  return {'methods': methods, 'superclass': class_node.attrib['extends']}
