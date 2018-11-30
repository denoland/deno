# Copyright 2013 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import difflib
import hashlib
import itertools
import json
import os
import sys
import zipfile


# When set and a difference is detected, a diff of what changed is printed.
PRINT_EXPLANATIONS = int(os.environ.get('PRINT_BUILD_EXPLANATIONS', 0))

# An escape hatch that causes all targets to be rebuilt.
_FORCE_REBUILD = int(os.environ.get('FORCE_REBUILD', 0))


def CallAndRecordIfStale(
    function, record_path=None, input_paths=None, input_strings=None,
    output_paths=None, force=False, pass_changes=False):
  """Calls function if outputs are stale.

  Outputs are considered stale if:
  - any output_paths are missing, or
  - the contents of any file within input_paths has changed, or
  - the contents of input_strings has changed.

  To debug which files are out-of-date, set the environment variable:
      PRINT_MD5_DIFFS=1

  Args:
    function: The function to call.
    record_path: Path to record metadata.
      Defaults to output_paths[0] + '.md5.stamp'
    input_paths: List of paths to calcualte an md5 sum on.
    input_strings: List of strings to record verbatim.
    output_paths: List of output paths.
    force: Whether to treat outputs as missing regardless of whether they
      actually are.
    pass_changes: Whether to pass a Changes instance to |function|.
  """
  assert record_path or output_paths
  input_paths = input_paths or []
  input_strings = input_strings or []
  output_paths = output_paths or []
  record_path = record_path or output_paths[0] + '.md5.stamp'

  assert record_path.endswith('.stamp'), (
      'record paths must end in \'.stamp\' so that they are easy to find '
      'and delete')

  new_metadata = _Metadata()
  new_metadata.AddStrings(input_strings)

  for path in input_paths:
    if _IsZipFile(path):
      entries = _ExtractZipEntries(path)
      new_metadata.AddZipFile(path, entries)
    else:
      new_metadata.AddFile(path, _Md5ForPath(path))

  old_metadata = None
  force = force or _FORCE_REBUILD
  missing_outputs = [x for x in output_paths if force or not os.path.exists(x)]
  # When outputs are missing, don't bother gathering change information.
  if not missing_outputs and os.path.exists(record_path):
    with open(record_path, 'r') as jsonfile:
      try:
        old_metadata = _Metadata.FromFile(jsonfile)
      except:  # pylint: disable=bare-except
        pass  # Not yet using new file format.

  changes = Changes(old_metadata, new_metadata, force, missing_outputs)
  if not changes.HasChanges():
    return

  if PRINT_EXPLANATIONS:
    print '=' * 80
    print 'Target is stale: %s' % record_path
    print changes.DescribeDifference()
    print '=' * 80

  args = (changes,) if pass_changes else ()
  function(*args)

  with open(record_path, 'w') as f:
    new_metadata.ToFile(f)


class Changes(object):
  """Provides and API for querying what changed between runs."""

  def __init__(self, old_metadata, new_metadata, force, missing_outputs):
    self.old_metadata = old_metadata
    self.new_metadata = new_metadata
    self.force = force
    self.missing_outputs = missing_outputs

  def _GetOldTag(self, path, subpath=None):
    return self.old_metadata and self.old_metadata.GetTag(path, subpath)

  def HasChanges(self):
    """Returns whether any changes exist."""
    return (self.force or
            not self.old_metadata or
            self.old_metadata.StringsMd5() != self.new_metadata.StringsMd5() or
            self.old_metadata.FilesMd5() != self.new_metadata.FilesMd5())

  def AddedOrModifiedOnly(self):
    """Returns whether the only changes were from added or modified (sub)files.

    No missing outputs, no removed paths/subpaths.
    """
    if (self.force or
        not self.old_metadata or
        self.old_metadata.StringsMd5() != self.new_metadata.StringsMd5()):
      return False
    if any(self.IterRemovedPaths()):
      return False
    for path in self.IterModifiedPaths():
      if any(self.IterRemovedSubpaths(path)):
        return False
    return True

  def IterAllPaths(self):
    """Generator for paths."""
    return self.new_metadata.IterPaths();

  def IterAllSubpaths(self, path):
    """Generator for subpaths."""
    return self.new_metadata.IterSubpaths(path);

  def IterAddedPaths(self):
    """Generator for paths that were added."""
    for path in self.new_metadata.IterPaths():
      if self._GetOldTag(path) is None:
        yield path

  def IterAddedSubpaths(self, path):
    """Generator for paths that were added within the given zip file."""
    for subpath in self.new_metadata.IterSubpaths(path):
      if self._GetOldTag(path, subpath) is None:
        yield subpath

  def IterRemovedPaths(self):
    """Generator for paths that were removed."""
    if self.old_metadata:
      for path in self.old_metadata.IterPaths():
        if self.new_metadata.GetTag(path) is None:
          yield path

  def IterRemovedSubpaths(self, path):
    """Generator for paths that were removed within the given zip file."""
    if self.old_metadata:
      for subpath in self.old_metadata.IterSubpaths(path):
        if self.new_metadata.GetTag(path, subpath) is None:
          yield subpath

  def IterModifiedPaths(self):
    """Generator for paths whose contents have changed."""
    for path in self.new_metadata.IterPaths():
      old_tag = self._GetOldTag(path)
      new_tag = self.new_metadata.GetTag(path)
      if old_tag is not None and old_tag != new_tag:
        yield path

  def IterModifiedSubpaths(self, path):
    """Generator for paths within a zip file whose contents have changed."""
    for subpath in self.new_metadata.IterSubpaths(path):
      old_tag = self._GetOldTag(path, subpath)
      new_tag = self.new_metadata.GetTag(path, subpath)
      if old_tag is not None and old_tag != new_tag:
        yield subpath

  def IterChangedPaths(self):
    """Generator for all changed paths (added/removed/modified)."""
    return itertools.chain(self.IterRemovedPaths(),
                           self.IterModifiedPaths(),
                           self.IterAddedPaths())

  def IterChangedSubpaths(self, path):
    """Generator for paths within a zip that were added/removed/modified."""
    return itertools.chain(self.IterRemovedSubpaths(path),
                           self.IterModifiedSubpaths(path),
                           self.IterAddedSubpaths(path))

  def DescribeDifference(self):
    """Returns a human-readable description of what changed."""
    if self.force:
      return 'force=True'
    elif self.missing_outputs:
      return 'Outputs do not exist:\n  ' + '\n  '.join(self.missing_outputs)
    elif self.old_metadata is None:
      return 'Previous stamp file not found.'

    if self.old_metadata.StringsMd5() != self.new_metadata.StringsMd5():
      ndiff = difflib.ndiff(self.old_metadata.GetStrings(),
                            self.new_metadata.GetStrings())
      changed = [s for s in ndiff if not s.startswith(' ')]
      return 'Input strings changed:\n  ' + '\n  '.join(changed)

    if self.old_metadata.FilesMd5() == self.new_metadata.FilesMd5():
      return "There's no difference."

    lines = []
    lines.extend('Added: ' + p for p in self.IterAddedPaths())
    lines.extend('Removed: ' + p for p in self.IterRemovedPaths())
    for path in self.IterModifiedPaths():
      lines.append('Modified: ' + path)
      lines.extend('  -> Subpath added: ' + p
                   for p in self.IterAddedSubpaths(path))
      lines.extend('  -> Subpath removed: ' + p
                   for p in self.IterRemovedSubpaths(path))
      lines.extend('  -> Subpath modified: ' + p
                   for p in self.IterModifiedSubpaths(path))
    if lines:
      return 'Input files changed:\n  ' + '\n  '.join(lines)
    return 'I have no idea what changed (there is a bug).'


class _Metadata(object):
  """Data model for tracking change metadata."""
  # Schema:
  # {
  #   "files-md5": "VALUE",
  #   "strings-md5": "VALUE",
  #   "input-files": [
  #     {
  #       "path": "path.jar",
  #       "tag": "{MD5 of entries}",
  #       "entries": [
  #         { "path": "org/chromium/base/Foo.class", "tag": "{CRC32}" }, ...
  #       ]
  #     }, {
  #       "path": "path.txt",
  #       "tag": "{MD5}",
  #     }
  #   ],
  #   "input-strings": ["a", "b", ...],
  # }
  def __init__(self):
    self._files_md5 = None
    self._strings_md5 = None
    self._files = []
    self._strings = []
    # Map of (path, subpath) -> entry. Created upon first call to _GetEntry().
    self._file_map = None

  @classmethod
  def FromFile(cls, fileobj):
    """Returns a _Metadata initialized from a file object."""
    ret = cls()
    obj = json.load(fileobj)
    ret._files_md5 = obj['files-md5']
    ret._strings_md5 = obj['strings-md5']
    ret._files = obj['input-files']
    ret._strings = obj['input-strings']
    return ret

  def ToFile(self, fileobj):
    """Serializes metadata to the given file object."""
    obj = {
        "files-md5": self.FilesMd5(),
        "strings-md5": self.StringsMd5(),
        "input-files": self._files,
        "input-strings": self._strings,
    }
    json.dump(obj, fileobj, indent=2)

  def _AssertNotQueried(self):
    assert self._files_md5 is None
    assert self._strings_md5 is None
    assert self._file_map is None

  def AddStrings(self, values):
    self._AssertNotQueried()
    self._strings.extend(str(v) for v in values)

  def AddFile(self, path, tag):
    """Adds metadata for a non-zip file.

    Args:
      path: Path to the file.
      tag: A short string representative of the file contents.
    """
    self._AssertNotQueried()
    self._files.append({
        'path': path,
        'tag': tag,
    })

  def AddZipFile(self, path, entries):
    """Adds metadata for a zip file.

    Args:
      path: Path to the file.
      entries: List of (subpath, tag) tuples for entries within the zip.
    """
    self._AssertNotQueried()
    tag = _ComputeInlineMd5(itertools.chain((e[0] for e in entries),
                                            (e[1] for e in entries)))
    self._files.append({
        'path': path,
        'tag': tag,
        'entries': [{"path": e[0], "tag": e[1]} for e in entries],
    })

  def GetStrings(self):
    """Returns the list of input strings."""
    return self._strings

  def FilesMd5(self):
    """Lazily computes and returns the aggregate md5 of input files."""
    if self._files_md5 is None:
      # Omit paths from md5 since temporary files have random names.
      self._files_md5 = _ComputeInlineMd5(
          self.GetTag(p) for p in sorted(self.IterPaths()))
    return self._files_md5

  def StringsMd5(self):
    """Lazily computes and returns the aggregate md5 of input strings."""
    if self._strings_md5 is None:
      self._strings_md5 = _ComputeInlineMd5(self._strings)
    return self._strings_md5

  def _GetEntry(self, path, subpath=None):
    """Returns the JSON entry for the given path / subpath."""
    if self._file_map is None:
      self._file_map = {}
      for entry in self._files:
        self._file_map[(entry['path'], None)] = entry
        for subentry in entry.get('entries', ()):
          self._file_map[(entry['path'], subentry['path'])] = subentry
    return self._file_map.get((path, subpath))

  def GetTag(self, path, subpath=None):
    """Returns the tag for the given path / subpath."""
    ret = self._GetEntry(path, subpath)
    return ret and ret['tag']

  def IterPaths(self):
    """Returns a generator for all top-level paths."""
    return (e['path'] for e in self._files)

  def IterSubpaths(self, path):
    """Returns a generator for all subpaths in the given zip.

    If the given path is not a zip file or doesn't exist, returns an empty
    iterable.
    """
    outer_entry = self._GetEntry(path)
    if not outer_entry:
      return ()
    subentries = outer_entry.get('entries', [])
    return (entry['path'] for entry in subentries)


def _UpdateMd5ForFile(md5, path, block_size=2**16):
  with open(path, 'rb') as infile:
    while True:
      data = infile.read(block_size)
      if not data:
        break
      md5.update(data)


def _UpdateMd5ForDirectory(md5, dir_path):
  for root, _, files in os.walk(dir_path):
    for f in files:
      _UpdateMd5ForFile(md5, os.path.join(root, f))


def _Md5ForPath(path):
  md5 = hashlib.md5()
  if os.path.isdir(path):
    _UpdateMd5ForDirectory(md5, path)
  else:
    _UpdateMd5ForFile(md5, path)
  return md5.hexdigest()


def _ComputeInlineMd5(iterable):
  """Computes the md5 of the concatenated parameters."""
  md5 = hashlib.md5()
  for item in iterable:
    md5.update(str(item))
  return md5.hexdigest()


def _IsZipFile(path):
  """Returns whether to treat the given file as a zip file."""
  # ijar doesn't set the CRC32 field.
  if path.endswith('.interface.jar'):
    return False
  return path[-4:] in ('.zip', '.apk', '.jar') or path.endswith('.srcjar')


def _ExtractZipEntries(path):
  """Returns a list of (path, CRC32) of all files within |path|."""
  entries = []
  with zipfile.ZipFile(path) as zip_file:
    for zip_info in zip_file.infolist():
      # Skip directories and empty files.
      if zip_info.CRC:
        entries.append(
            (zip_info.filename, zip_info.CRC + zip_info.compress_type))
  return entries
