# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Creates a archive manifest used for Fuchsia package generation.

Arguments:
  root_dir: The absolute path to the Chromium source tree root.

  out_dir: The absolute path to the Chromium build directory.

  app_name: The filename of the package's executable target.

  runtime_deps: The path to the GN runtime deps file.

  output_path: The path of the manifest file which will be written.
"""

import json
import os
import re
import subprocess
import sys
import tempfile


def ReadDynamicLibDeps(paths):
  """Returns a list of NEEDED libraries read from a binary's ELF header."""

  LIBRARY_RE = re.compile(r'.*\(NEEDED\)\s+Shared library: \[(?P<lib>.*)\]')
  elfinfo = subprocess.check_output(['readelf', '-d'] + paths,
                                    stderr=open(os.devnull, 'w'))
  libs = []
  for line in elfinfo.split('\n'):
    match = LIBRARY_RE.match(line.rstrip())
    if match:
      lib = match.group('lib')

      # libc.so is an alias for ld.so.1 .
      if lib == 'libc.so':
        lib = 'ld.so.1'

      # Skip libzircon.so, as it is supplied by the OS loader.
      if lib != 'libzircon.so':
        libs.append(lib)

  return libs


def ComputeTransitiveLibDeps(executable_path, available_libs):
  """Returns a set representing the library dependencies of |executable_path|,
  the dependencies of its dependencies, and so on.

  A list of candidate library filesystem paths is passed using |available_libs|
  to help with resolving full paths from the short ELF header filenames."""

  # Stack of binaries (libraries, executables) awaiting traversal.
  to_visit = [executable_path]

  # The computed set of visited transitive dependencies.
  deps = set()

  while to_visit:
    deps = deps.union(to_visit)

    # Resolve the full paths for all of |cur_path|'s NEEDED libraries.
    dep_paths = {available_libs[dep]
                 for dep in ReadDynamicLibDeps(list(to_visit))}

    # Add newly discovered dependencies to the pending traversal stack.
    to_visit = dep_paths.difference(deps)

  return deps


def EnumerateDirectoryFiles(path):
  """Returns a flattened list of all files contained under |path|."""

  output = set()
  for dirname, _, files in os.walk(path):
    output = output.union({os.path.join(dirname, f) for f in files})
  return output


def MakePackagePath(file_path, roots):
  """Computes a path for |file_path| that is relative to one of the directory
  paths in |roots|.

  file_path: The absolute file path to relativize.
  roots: A list of absolute directory paths which may serve as a relative root
         for |file_path|. At least one path must contain |file_path|.
         Overlapping roots are permitted; the deepest matching root will be
         chosen.

  Examples:

  >>> MakePackagePath('/foo/bar.txt', ['/foo/'])
  'bar.txt'

  >>> MakePackagePath('/foo/dir/bar.txt', ['/foo/'])
  'dir/bar.txt'

  >>> MakePackagePath('/foo/out/Debug/bar.exe', ['/foo/', '/foo/out/Debug/'])
  'bar.exe'
  """

  # Prevents greedily matching against a shallow path when a deeper, better
  # matching path exists.
  roots.sort(key=len, reverse=True)

  for next_root in roots:
    if not next_root.endswith(os.sep):
      next_root += os.sep

    if file_path.startswith(next_root):
      relative_path = file_path[len(next_root):]

      # Move all dynamic libraries (ending in .so or .so.<number>) to lib/.
      if re.search('.*\.so(\.\d+)?$', file_path):
        relative_path = 'lib/' + os.path.basename(relative_path)

      return relative_path

  raise Exception('Error: no matching root paths found for \'%s\'.' % file_path)


def _GetStrippedPath(bin_path):
  """Finds the stripped version of the binary |bin_path| in the build
  output directory."""

  # Skip the resolution step for binaries that don't have stripped counterparts,
  # like system libraries or other libraries built outside the Chromium build.
  if not '.unstripped' in bin_path:
    return bin_path

  return os.path.normpath(os.path.join(bin_path,
                                       os.path.pardir,
                                       os.path.pardir,
                                       os.path.basename(bin_path)))


def _IsBinary(path):
  """Checks if the file at |path| is an ELF executable by inspecting its FourCC
  header."""

  with open(path, 'rb') as f:
    file_tag = f.read(4)
  return file_tag == '\x7fELF'


def BuildManifest(root_dir, out_dir, app_name, app_filename,
                  sandbox_policy_path, runtime_deps_file, depfile_path,
                  dynlib_paths, output_path):
  with open(output_path, 'w') as manifest, open(depfile_path, 'w') as depfile:
    # Process the runtime deps file for file paths, recursively walking
    # directories as needed. File paths are stored in absolute form,
    # so that MakePackagePath() may relativize to either the source root or
    # output directory.
    # runtime_deps may contain duplicate paths, so use a set for
    # de-duplication.
    expanded_files = set()
    for next_path in open(runtime_deps_file, 'r'):
      next_path = next_path.strip()
      if os.path.isdir(next_path):
        for root, _, files in os.walk(next_path):
          for current_file in files:
            if current_file.startswith('.'):
              continue
            expanded_files.add(os.path.abspath(
                os.path.join(root, current_file)))
      else:
        expanded_files.add(os.path.abspath(next_path))

    # Get set of dist libraries available for dynamic linking.
    dist_libs = set()
    for next_dir in dynlib_paths.split(','):
      dist_libs = dist_libs.union(EnumerateDirectoryFiles(next_dir))

    # Compute the set of dynamic libraries used by the application or its
    # transitive dependencies (dist libs and components), and merge the result
    # with |expanded_files| so that they are included in the manifest.

    # TODO(https://crbug.com/861931): Temporarily just include all |dist_libs|.
    #expanded_files = expanded_files.union(
    #    ComputeTransitiveLibDeps(
    #        app_filename,
    #        {os.path.basename(f): f for f in expanded_files.union(dist_libs)}))
    expanded_files = expanded_files.union(dist_libs)

    # Format and write out the manifest contents.
    gen_dir = os.path.join(out_dir, "gen")
    app_found = False
    for current_file in expanded_files:
      if _IsBinary(current_file):
        current_file = _GetStrippedPath(current_file)

      in_package_path = MakePackagePath(os.path.join(out_dir, current_file),
                                        [gen_dir, root_dir, out_dir])
      if in_package_path == app_filename:
        app_found = True

      # The source path is relativized so that it can be used on multiple
      # environments with differing parent directory structures,
      # e.g. builder bots and swarming clients.
      manifest.write('%s=%s\n' % (in_package_path,
                                  os.path.relpath(current_file, out_dir)))

    if not app_found:
      raise Exception('Could not locate executable inside runtime_deps.')

    # Write meta/package manifest file.
    with open(os.path.join(os.path.dirname(output_path), 'package'), 'w') \
        as package_json:
      json.dump({'version': '0', 'name': app_name}, package_json)
      manifest.write('meta/package=%s\n' %
                   os.path.relpath(package_json.name, out_dir))

    # Write component manifest file.
    with open(os.path.join(os.path.dirname(output_path),
                           app_name + '.cmx'), 'w') as component_manifest_file:
      component_manifest = {
          'program': { 'binary': app_filename },
          'sandbox': json.load(open(sandbox_policy_path, 'r')),
      }
      json.dump(component_manifest, component_manifest_file)
      manifest.write('meta/%s=%s\n' %
                     (os.path.basename(component_manifest_file.name),
                      os.path.relpath(component_manifest_file.name, out_dir)))

    depfile.write(
        "%s: %s" % (os.path.relpath(output_path, out_dir),
                    " ".join([os.path.relpath(f, out_dir)
                              for f in expanded_files])))
  return 0


if __name__ == '__main__':
  sys.exit(BuildManifest(*sys.argv[1:]))
