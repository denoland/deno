# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import argparse
import collections
import contextlib
import os
import re
import shutil
import sys
import tempfile
from xml.etree import ElementTree

import util.build_utils as build_utils

_SOURCE_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), '..', '..', '..', '..'))
# Import jinja2 from third_party/jinja2
sys.path.insert(1, os.path.join(_SOURCE_ROOT, 'third_party'))
from jinja2 import Template # pylint: disable=F0401


EMPTY_ANDROID_MANIFEST_PATH = os.path.join(
    _SOURCE_ROOT, 'build', 'android', 'AndroidManifest.xml')


# A variation of this lists also exists in:
# //base/android/java/src/org/chromium/base/LocaleUtils.java
# //ui/android/java/src/org/chromium/base/LocalizationUtils.java
CHROME_TO_ANDROID_LOCALE_MAP = {
    'en-GB': 'en-rGB',
    'en-US': 'en-rUS',
    'es-419': 'es-rUS',
    'fil': 'tl',
    'he': 'iw',
    'id': 'in',
    'pt-PT': 'pt-rPT',
    'pt-BR': 'pt-rBR',
    'yi': 'ji',
    'zh-CN': 'zh-rCN',
    'zh-TW': 'zh-rTW',
}

# Represents a line from a R.txt file.
_TextSymbolEntry = collections.namedtuple('RTextEntry',
    ('java_type', 'resource_type', 'name', 'value'))


def CreateResourceInfoFile(files_to_zip, zip_path):
  """Given a mapping of archive paths to their source, write an info file.

  The info file contains lines of '{archive_path},{source_path}' for ease of
  parsing. Assumes that there is no comma in the file names.

  Args:
    files_to_zip: Dict mapping path in the zip archive to original source.
    zip_path: Path where the zip file ends up, this is where the info file goes.
  """
  info_file_path = zip_path + '.info'
  with open(info_file_path, 'w') as info_file:
    for archive_path, source_path in files_to_zip.iteritems():
      info_file.write('{},{}\n'.format(archive_path, source_path))


def _ParseTextSymbolsFile(path, fix_package_ids=False):
  """Given an R.txt file, returns a list of _TextSymbolEntry.

  Args:
    path: Input file path.
    fix_package_ids: if True, all packaged IDs read from the file
      will be fixed to 0x7f.
  Returns:
    A list of _TextSymbolEntry instances.
  Raises:
    Exception: An unexpected line was detected in the input.
  """
  ret = []
  with open(path) as f:
    for line in f:
      m = re.match(r'(int(?:\[\])?) (\w+) (\w+) (.+)$', line)
      if not m:
        raise Exception('Unexpected line in R.txt: %s' % line)
      java_type, resource_type, name, value = m.groups()
      if fix_package_ids:
        value = _FixPackageIds(value)
      ret.append(_TextSymbolEntry(java_type, resource_type, name, value))
  return ret


def _FixPackageIds(resource_value):
  # Resource IDs for resources belonging to regular APKs have their first byte
  # as 0x7f (package id). However with webview, since it is not a regular apk
  # but used as a shared library, aapt is passed the --shared-resources flag
  # which changes some of the package ids to 0x02 and 0x00.  This function just
  # normalises all package ids to 0x7f, which the generated code in R.java
  # changes to the correct package id at runtime.
  # resource_value is a string with either, a single value '0x12345678', or an
  # array of values like '{ 0xfedcba98, 0x01234567, 0x56789abc }'
  return re.sub(r'0x(?!01)\d\d', r'0x7f', resource_value)


def _GetRTxtResourceNames(r_txt_path):
  """Parse an R.txt file and extract the set of resource names from it."""
  result = set()
  for entry in _ParseTextSymbolsFile(r_txt_path):
    result.add(entry.name)
  return result


class RJavaBuildOptions:
  """A class used to model the various ways to build an R.java file.

  This is used to control which resource ID variables will be final or
  non-final, and whether an onResourcesLoaded() method will be generated
  to adjust the non-final ones, when the corresponding library is loaded
  at runtime.

  Note that by default, all resources are final, and there is no
  method generated, which corresponds to calling ExportNoResources().
  """
  def __init__(self):
    self.has_constant_ids = True
    self.resources_whitelist = None
    self.has_on_resources_loaded = False
    self.export_const_styleable = False

  def ExportNoResources(self):
    """Make all resource IDs final, and don't generate a method."""
    self.has_constant_ids = True
    self.resources_whitelist = None
    self.has_on_resources_loaded = False
    self.export_const_styleable = False

  def ExportAllResources(self):
    """Make all resource IDs non-final in the R.java file."""
    self.has_constant_ids = False
    self.resources_whitelist = None

  def ExportSomeResources(self, r_txt_file_path):
    """Only select specific resource IDs to be non-final.

    Args:
      r_txt_file_path: The path to an R.txt file. All resources named
        int it will be non-final in the generated R.java file, all others
        will be final.
    """
    self.has_constant_ids = True
    self.resources_whitelist = _GetRTxtResourceNames(r_txt_file_path)

  def ExportAllStyleables(self):
    """Make all styleable constants non-final, even non-resources ones.

    Resources that are styleable but not of int[] type are not actually
    resource IDs but constants. By default they are always final. Call this
    method to make them non-final anyway in the final R.java file.
    """
    self.export_const_styleable = True

  def GenerateOnResourcesLoaded(self):
    """Generate an onResourcesLoaded() method.

    This Java method will be called at runtime by the framework when
    the corresponding library (which includes the R.java source file)
    will be loaded at runtime. This corresponds to the --shared-resources
    or --app-as-shared-lib flags of 'aapt package'.
    """
    self.has_on_resources_loaded = True

  def _IsResourceFinal(self, entry):
    """Determines whether a resource should be final or not.

  Args:
    entry: A _TextSymbolEntry instance.
  Returns:
    True iff the corresponding entry should be final.
  """
    if entry.resource_type == 'styleable' and entry.java_type != 'int[]':
      # A styleable constant may be exported as non-final after all.
      return not self.export_const_styleable
    elif not self.has_constant_ids:
      # Every resource is non-final
      return False
    elif not self.resources_whitelist:
      # No whitelist means all IDs are non-final.
      return True
    else:
      # Otherwise, only those in the
      return entry.name not in self.resources_whitelist


def CreateRJavaFiles(srcjar_dir, package, main_r_txt_file,
                     extra_res_packages, extra_r_txt_files,
                     rjava_build_options):
  """Create all R.java files for a set of packages and R.txt files.

  Args:
    srcjar_dir: The top-level output directory for the generated files.
    package: Top-level package name.
    main_r_txt_file: The main R.txt file containing the valid values
      of _all_ resource IDs.
    extra_res_packages: A list of extra package names.
    extra_r_txt_files: A list of extra R.txt files. One per item in
      |extra_res_packages|. Note that all resource IDs in them will be ignored,
      |and replaced by the values extracted from |main_r_txt_file|.
    rjava_build_options: An RJavaBuildOptions instance that controls how
      exactly the R.java file is generated.
  Raises:
    Exception if a package name appears several times in |extra_res_packages|
  """
  assert len(extra_res_packages) == len(extra_r_txt_files), \
         'Need one R.txt file per package'

  packages = list(extra_res_packages)
  r_txt_files = list(extra_r_txt_files)

  if package and package not in packages:
    # Sometimes, an apk target and a resources target share the same
    # AndroidManifest.xml and thus |package| will already be in |packages|.
    packages.append(package)
    r_txt_files.append(main_r_txt_file)

  # Map of (resource_type, name) -> Entry.
  # Contains the correct values for resources.
  all_resources = {}
  for entry in _ParseTextSymbolsFile(main_r_txt_file, fix_package_ids=True):
    all_resources[(entry.resource_type, entry.name)] = entry

  # Map of package_name->resource_type->entry
  resources_by_package = (
      collections.defaultdict(lambda: collections.defaultdict(list)))
  # Build the R.java files using each package's R.txt file, but replacing
  # each entry's placeholder value with correct values from all_resources.
  for package, r_txt_file in zip(packages, r_txt_files):
    if package in resources_by_package:
      raise Exception(('Package name "%s" appeared twice. All '
                       'android_resources() targets must use unique package '
                       'names, or no package name at all.') % package)
    resources_by_type = resources_by_package[package]
    # The sub-R.txt files have the wrong values at this point. Read them to
    # figure out which entries belong to them, but use the values from the
    # main R.txt file.
    for entry in _ParseTextSymbolsFile(r_txt_file):
      entry = all_resources.get((entry.resource_type, entry.name))
      # For most cases missing entry here is an error. It means that some
      # library claims to have or depend on a resource that isn't included into
      # the APK. There is one notable exception: Google Play Services (GMS).
      # GMS is shipped as a bunch of AARs. One of them - basement - contains
      # R.txt with ids of all resources, but most of the resources are in the
      # other AARs. However, all other AARs reference their resources via
      # basement's R.java so the latter must contain all ids that are in its
      # R.txt. Most targets depend on only a subset of GMS AARs so some
      # resources are missing, which is okay because the code that references
      # them is missing too. We can't get an id for a resource that isn't here
      # so the only solution is to skip the resource entry entirely.
      #
      # We can verify that all entries referenced in the code were generated
      # correctly by running Proguard on the APK: it will report missing
      # fields.
      if entry:
        resources_by_type[entry.resource_type].append(entry)

  for package, resources_by_type in resources_by_package.iteritems():
    _CreateRJavaSourceFile(srcjar_dir, package, resources_by_type,
                           rjava_build_options)


def _CreateRJavaSourceFile(srcjar_dir, package, resources_by_type,
                           rjava_build_options):
  """Generates an R.java source file."""
  package_r_java_dir = os.path.join(srcjar_dir, *package.split('.'))
  build_utils.MakeDirectory(package_r_java_dir)
  package_r_java_path = os.path.join(package_r_java_dir, 'R.java')
  java_file_contents = _RenderRJavaSource(package, resources_by_type,
                                          rjava_build_options)
  with open(package_r_java_path, 'w') as f:
    f.write(java_file_contents)


# Resource IDs inside resource arrays are sorted. Application resource IDs start
# with 0x7f but system resource IDs start with 0x01 thus system resource ids are
# always at the start of the array. This function finds the index of the first
# non system resource id to be used for package ID rewriting (we should not
# rewrite system resource ids).
def _GetNonSystemIndex(entry):
  """Get the index of the first application resource ID within a resource
  array."""
  res_ids = re.findall(r'0x[0-9a-f]{8}', entry.value)
  for i, res_id in enumerate(res_ids):
    if res_id.startswith('0x7f'):
      return i
  return len(res_ids)


def _RenderRJavaSource(package, resources_by_type, rjava_build_options):
  """Render an R.java source file. See _CreateRJaveSourceFile for args info."""
  final_resources_by_type = collections.defaultdict(list)
  non_final_resources_by_type = collections.defaultdict(list)
  for res_type, resources in resources_by_type.iteritems():
    for entry in resources:
      # Entries in stylable that are not int[] are not actually resource ids
      # but constants.
      if rjava_build_options._IsResourceFinal(entry):
        final_resources_by_type[res_type].append(entry)
      else:
        non_final_resources_by_type[res_type].append(entry)

  # Keep these assignments all on one line to make diffing against regular
  # aapt-generated files easier.
  create_id = ('{{ e.resource_type }}.{{ e.name }} ^= packageIdTransform;')
  create_id_arr = ('{{ e.resource_type }}.{{ e.name }}[i] ^='
                   ' packageIdTransform;')
  for_loop_condition  = ('int i = {{ startIndex(e) }}; i < '
                         '{{ e.resource_type }}.{{ e.name }}.length; ++i')

  # Here we diverge from what aapt does. Because we have so many
  # resources, the onResourcesLoaded method was exceeding the 64KB limit that
  # Java imposes. For this reason we split onResourcesLoaded into different
  # methods for each resource type.
  template = Template("""/* AUTO-GENERATED FILE.  DO NOT MODIFY. */

package {{ package }};

public final class R {
    private static boolean sResourcesDidLoad;
    {% for resource_type in resource_types %}
    public static final class {{ resource_type }} {
        {% for e in final_resources[resource_type] %}
        public static final {{ e.java_type }} {{ e.name }} = {{ e.value }};
        {% endfor %}
        {% for e in non_final_resources[resource_type] %}
            {% if e.value != '0' %}
        public static {{ e.java_type }} {{ e.name }} = {{ e.value }};
            {% else %}
        public static {{ e.java_type }} {{ e.name }};
            {% endif %}
        {% endfor %}
    }
    {% endfor %}
    {% if has_on_resources_loaded %}
    public static void onResourcesLoaded(int packageId) {
        assert !sResourcesDidLoad;
        sResourcesDidLoad = true;
        int packageIdTransform = (packageId ^ 0x7f) << 24;
        {% for resource_type in resource_types %}
        onResourcesLoaded{{ resource_type|title }}(packageIdTransform);
        {% for e in non_final_resources[resource_type] %}
        {% if e.java_type == 'int[]' %}
        for(""" + for_loop_condition + """) {
            """ + create_id_arr + """
        }
        {% endif %}
        {% endfor %}
        {% endfor %}
    }
    {% for res_type in resource_types %}
    private static void onResourcesLoaded{{ res_type|title }} (
            int packageIdTransform) {
        {% for e in non_final_resources[res_type] %}
        {% if res_type != 'styleable' and e.java_type != 'int[]' %}
        """ + create_id + """
        {% endif %}
        {% endfor %}
    }
    {% endfor %}
    {% endif %}
}
""", trim_blocks=True, lstrip_blocks=True)

  return template.render(
      package=package,
      resource_types=sorted(resources_by_type),
      has_on_resources_loaded=rjava_build_options.has_on_resources_loaded,
      final_resources=final_resources_by_type,
      non_final_resources=non_final_resources_by_type,
      startIndex=_GetNonSystemIndex)


def ExtractPackageFromManifest(manifest_path):
  """Extract package name from Android manifest file."""
  doc = ElementTree.parse(manifest_path)
  return doc.getroot().get('package')


def ExtractDeps(dep_zips, deps_dir):
  """Extract a list of resource dependency zip files.

  Args:
     dep_zips: A list of zip file paths, each one will be extracted to
       a subdirectory of |deps_dir|, named after the zip file (e.g.
       '/some/path/foo.zip' -> '{deps_dir}/foo/').
    deps_dir: Top-level extraction directory.
  Returns:
    The list of all sub-directory paths, relative to |deps_dir|.
  Raises:
    Exception: If a sub-directory already exists with the same name before
      extraction.
  """
  dep_subdirs = []
  for z in dep_zips:
    subdir = os.path.join(deps_dir, os.path.basename(z))
    if os.path.exists(subdir):
      raise Exception('Resource zip name conflict: ' + os.path.basename(z))
    build_utils.ExtractAll(z, path=subdir)
    dep_subdirs.append(subdir)
  return dep_subdirs


class _ResourceBuildContext(object):
  """A temporary directory for packaging and compiling Android resources."""
  def __init__(self):
    """Initialized the context."""
    # The top-level temporary directory.
    self.temp_dir = tempfile.mkdtemp()
    # A location to store resources extracted form dependency zip files.
    self.deps_dir = os.path.join(self.temp_dir, 'deps')
    os.mkdir(self.deps_dir)
    # A location to place aapt-generated files.
    self.gen_dir = os.path.join(self.temp_dir, 'gen')
    os.mkdir(self.gen_dir)
    # Location of the generated R.txt file.
    self.r_txt_path = os.path.join(self.gen_dir, 'R.txt')
    # A location to place generated R.java files.
    self.srcjar_dir = os.path.join(self.temp_dir, 'java')
    os.mkdir(self.srcjar_dir)

  def Close(self):
    """Close the context and destroy all temporary files."""
    shutil.rmtree(self.temp_dir)


@contextlib.contextmanager
def BuildContext():
  """Generator for a _ResourceBuildContext instance."""
  try:
    context = _ResourceBuildContext()
    yield context
  finally:
    context.Close()


def ResourceArgsParser():
  """Create an argparse.ArgumentParser instance with common argument groups.

  Returns:
    A tuple of (parser, in_group, out_group) corresponding to the parser
    instance, and the input and output argument groups for it, respectively.
  """
  parser = argparse.ArgumentParser(description=__doc__)

  input_opts = parser.add_argument_group('Input options')
  output_opts = parser.add_argument_group('Output options')

  build_utils.AddDepfileOption(output_opts)

  input_opts.add_argument('--include-resources', required=True, action="append",
                        help='Paths to arsc resource files used to link '
                             'against. Can be specified multiple times.')

  input_opts.add_argument('--aapt-path', required=True,
                         help='Path to the Android aapt tool')

  input_opts.add_argument('--aapt2-path',
                          help='Path to the Android aapt2 tool. If in different'
                          ' directory from --aapt-path.')

  input_opts.add_argument('--dependencies-res-zips', required=True,
                    help='Resources zip archives from dependents. Required to '
                         'resolve @type/foo references into dependent '
                         'libraries.')

  input_opts.add_argument(
      '--r-text-in',
       help='Path to pre-existing R.txt. Its resource IDs override those found '
            'in the aapt-generated R.txt when generating R.java.')

  input_opts.add_argument(
      '--extra-res-packages',
      help='Additional package names to generate R.java files for.')

  input_opts.add_argument(
      '--extra-r-text-files',
      help='For each additional package, the R.txt file should contain a '
           'list of resources to be included in the R.java file in the format '
           'generated by aapt.')

  return (parser, input_opts, output_opts)


def HandleCommonOptions(options):
  """Handle common command-line options after parsing.

  Args:
    options: the result of parse_args() on the parser returned by
        ResourceArgsParser(). This function updates a few common fields.
  """
  options.include_resources = [build_utils.ParseGnList(r) for r in
                               options.include_resources]
  # Flatten list of include resources list to make it easier to use.
  options.include_resources = [r for resources in options.include_resources
                               for r in resources]

  options.dependencies_res_zips = (
      build_utils.ParseGnList(options.dependencies_res_zips))

  # Don't use [] as default value since some script explicitly pass "".
  if options.extra_res_packages:
    options.extra_res_packages = (
        build_utils.ParseGnList(options.extra_res_packages))
  else:
    options.extra_res_packages = []

  if options.extra_r_text_files:
    options.extra_r_text_files = (
        build_utils.ParseGnList(options.extra_r_text_files))
  else:
    options.extra_r_text_files = []

  if not options.aapt2_path:
    options.aapt2_path = options.aapt_path + '2'
