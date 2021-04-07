#!/usr/bin/env python3
# This script is copied from:
# https://github.com/MestreLion/git-tools/blob/957810b/git-restore-mtime
#
# git-restore-mtime - Change mtime of files based on commit date of last change
#
#    Copyright (C) 2012 Rodrigo Silva (MestreLion) <linux@rodrigosilva.com>
#
#    This program is free software: you can redistribute it and/or modify
#    it under the terms of the GNU General Public License as published by
#    the Free Software Foundation, either version 3 of the License, or
#    (at your option) any later version.
#
#    This program is distributed in the hope that it will be useful,
#    but WITHOUT ANY WARRANTY; without even the implied warranty of
#    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
#    GNU General Public License for more details.
#
#    You should have received a copy of the GNU General Public License
#    along with this program. See <http://www.gnu.org/licenses/gpl.html>
#
"""
Change the modification time (mtime) of all files in work tree, based on the
date of the most recent commit that modified the file.

Useful prior to generating release tarballs, so each file is archived with a
date that is similar to the date when the file was actually last modified,
assuming the actual modification date and its commit date are close.

Ignores by default all ignored and untracked files, and also refuses to work
on trees with uncommitted changes.
"""

# TODO:
# - Add -z on git whatchanged/ls-files, so we don't deal with filename decoding/OS normalization
# - When Python is bumped to 3.7, use text instead of universal_newlines on subprocess
# - Update "Statistics for some large projects" with modern hardware and repositories.
# - Create a README.md for git-restore-mtime alone. It deserves extensive documentation
#   - Move Statistics there

# FIXME:
# - When current dir is outside the worktree, e.g. using --work-tree, `git ls-files`
#   assume any relative pathspecs are to worktree root, not the current dir. As such,
#   relative pathspecs may not work.
# - Renames and mode changes should not change file mtime:
#   - Must check on status 'R100' and mode changes with same blobs
#   - Should require status to be (A, C, M, R<100, T). D will never be processed as
#     filelist is a subset of lsfileslist.
# - Check file (A, D) for the directory mtime is not sufficient:
#   - Renames also change dir mtime, unless rename was on a parent dir
#   - If most recent change of all files in a dir was a Modification (M),
#     dir might not be touched at all.
#   - Dirs containing only subdirectories but no direct files will also
#     not be touched. They're files' [grand]parent dir, but never their dirname().
#   - Some solutions:
#     - After files done, perform some dir processing for missing dirs, finding latest
#       file (A, D, R)
#     - Simple approach: dir mtime is the most recent child (dir or file) mtime
#     - Use a virtual concept of "created at most at" to fill missing info, bubble up
#       to parents and grandparents
#   - When handling [grand]parent dirs, stay inside <pathspec>
# - Better handling of merge commits. `-m` is plain *wrong*. `-c/--cc` is perfect, but
#   painfully slow. First pass without merge commits is not accurate. Maybe add a new
#   `--accurate` mode for `--cc`?

if __name__ != "__main__":
    raise ImportError("{} should not be used as a module.".format(__name__))

import argparse
import logging
import os.path
import shlex
import subprocess
import sys
import time


# Update symlinks only if the OS supports not following them
UPDATE_SYMLINKS = bool(os.utime in getattr(os, 'supports_follow_symlinks', []))
STEPMISSING = 100


# Command-line interface ######################################################

def parse_args():
    parser = argparse.ArgumentParser(
        description="""Restore original modification time of files based on the date of the
        most recent commit that modified them. Useful when generating release tarballs.""")

    group = parser.add_mutually_exclusive_group()
    group.add_argument('--quiet', '-q', dest='loglevel',
        action="store_const", const=logging.WARNING, default=logging.INFO,
        help="Suppress informative messages and summary statistics.")
    group.add_argument('--verbose', '-v', action="count",
        help="Print additional information for each processed file.")

    parser.add_argument('--git-dir', dest='gitdir', metavar="GITDIR",
        help="""Path to the git repository, by default auto-discovered by git by searching
                the current directory and its parents for a .git/ subfolder.""")

    parser.add_argument('--work-tree', dest='workdir', metavar="WORKTREE",
        help="""Path to the work tree root, by default the parent of GITDIR if it was
                automatically discovered, or the current directory if GITDIR was set.""")

    parser.add_argument('--force', '-f', action="store_true",
        help="Force execution on trees with uncommitted changes.")

    parser.add_argument('--merge', '-m', action="store_true",
        help="""Include merge commits. Leads to more recent mtimes and more files per
        commit, thus with the same mtime (which may or may not be what you want). Including
        merge commits may lead to fewer commits being evaluated (all files are found sooner),
        which improves performance, sometimes substantially. But, as merge commits are
        usually huge, processing them may also take longer, sometimes substantially.
        By default merge logs are only used for files missing from regular commit logs.""")

    parser.add_argument('--first-parent', action="store_true",
        help="""Consider only the first parent, the "main branch", when parsing merge
        commit logs. Only effective when merge commits are included in the log, either
        by --merge or to find missing files after first log parse. See --skip-missing.""")

    parser.add_argument('--skip-missing', '-s',
        action="store_false", default=True, dest="missing",
        help="""Do not try to find missing files. If some files were not found in regular
        commit logs, by default it re-tries using merge commit logs for these files (if
        --merge was not already used). This option disables this behavior, which may slightly
        improve performance, but files found only in merge commits will not be updated.""")

    parser.add_argument('--no-directories', '-D',
        action="store_false", default=True, dest='dirs',
        help="""Do not update directory mtime for files created, renamed or deleted in it.
        Note: just modifying a file will not update its directory mtime.""")

    parser.add_argument('--test', '-t', action="store_true", default=False,
        help="Test run: do not actually update any file")

    parser.add_argument('--commit-time', '-c',
        action='store_true', default=False, dest='commit_time',
        help="Use commit time instead of author time")

    parser.add_argument('--oldest-time', '-o',
        action='store_true', default=False, dest='reverse_order',
        help="""Set the mtime to the time of the first commit to mention a given file
        instead of the most recent. This works by reversing the order in which the git
        log is processed (i.e. from the oldest to the most recent commit on the current
        branch, instead of from most recent to oldest). This may result in incorrect
        behaviour if there are multiple files which have been renamed with the same name
        in the current branch's history.""")

    parser.add_argument('--skip-older-than', metavar='SECONDS', type=int,
        help="""Do not modify files that are older than %(metavar)s.
        It can significantly improve performance if fewer files are processed.
        Useful on CI builds, which can eventually switch workspace to different branch,
        but mostly performs builds on the same one (e.g. master).""")

    parser.add_argument('pathspec', nargs='*', metavar='PATH',
        help="""Only modify paths matching PATH, directories or files, relative to current
        directory. Default is to modify all files handled by git, ignoring untracked files
        and submodules.""")

    return parser.parse_args()


# Helper functions ############################################################

def setup_logging(args_):
    logging.TRACE = TRACE = logging.DEBUG // 2
    logging.Logger.trace = lambda _, m, *a, **k: _.log(TRACE, m, *a, **k)
    level = ((args_.verbose and max(TRACE, logging.DEBUG // args_.verbose))
             or args_.loglevel)
    logging.basicConfig(level=level, format='%(message)s')
    return logging.getLogger()


def normalize(path):
    r"""Normalize paths from git, handling non-ASCII characters.

    Git for Windows, as of v1.7.10, stores paths as UTF-8 normalization form C. If path
    contains non-ASCII or non-printable chars it outputs the UTF-8 in octal-escaped
    notation, double-quoting the whole path. Double-quotes and backslashes are also escaped.

    https://git-scm.com/docs/git-config#Documentation/git-config.txt-corequotePath
    https://github.com/msysgit/msysgit/wiki/Git-for-Windows-Unicode-Support
    https://github.com/git/git/blob/master/Documentation/i18n.txt

    Example on git output, this function reverts this:
    r'back\slash_double"quote_açaí' -> r'"back\\slash_double\"quote_a\303\247a\303\255"'
    """
    if path and path[0] == '"':
        # Python 2: path = path[1:-1].decode("string-escape")
        # Python 3: https://stackoverflow.com/a/46650050/624066
        path = (path[1:-1]                 # Remove enclosing double quotes
                .encode('latin1')          # Convert to bytes, required 'unicode-escape'
                .decode('unicode-escape')  # Perform the actual octal-escaping decode
                .encode('latin1')          # 1:1 mapping to bytes, forming UTF-8 encoding
                .decode('utf8'))           # Decode from UTF-8
    # Make sure the slash matches the OS; for Windows we need a backslash
    return os.path.normpath(path)


if UPDATE_SYMLINKS:
    def touch(path, mtime, test=False):
        """The actual mtime update"""
        if test: return
        os.utime(path, (mtime, mtime), follow_symlinks=False)
else:
    def touch(path, mtime, test=False):
        """The actual mtime update"""
        if test: return
        os.utime(path, (mtime, mtime))


def isodate(secs):
    return time.strftime('%Y-%m-%d %H:%M:%S', time.localtime(secs))


# Git class and parselog(), the heart of the script ###########################

class Git:
    def __init__(self, workdir=None, gitdir=None):
        self.gitcmd = ['git']
        if workdir: self.gitcmd.extend(('--work-tree', workdir))
        if gitdir:  self.gitcmd.extend(('--git-dir',   gitdir))
        self.workdir, self.gitdir = self._repodirs()

    def ls_files(self, pathlist=None):
        return (normalize(_) for _ in self._run('ls-files --full-name', pathlist))

    def is_dirty(self):
        return bool(self._run('diff --no-ext-diff --quiet', output=False))

    def log(self, merge=False, first_parent=False, commit_time=False, reverse_order=False,
            pathlist=None):
        cmd = 'whatchanged --pretty={}'.format('%ct' if commit_time else '%at')
        if merge:         cmd += ' -m'
        if first_parent:  cmd += ' --first-parent'
        if reverse_order: cmd += ' --reverse'
        return self._run(cmd, pathlist)

    def _repodirs(self):
        return (os.path.normpath(_) for _ in
                self._run('rev-parse --show-toplevel --absolute-git-dir', check=True))

    def _run(self, cmdstr, pathlist=None, output=True, check=False):
        cmdlist = self.gitcmd + shlex.split(cmdstr)
        if pathlist:
            cmdlist.append('--')
            cmdlist.extend(pathlist)
        log.trace("Executing: %s", ' '.join(cmdlist))
        if not output:
            return subprocess.call(cmdlist)
        if check:
            try:
                stdout = subprocess.check_output(cmdlist, universal_newlines=True)
                return stdout.splitlines()
            except subprocess.CalledProcessError as e:
                raise self.Error(e.returncode, e.cmd, e.output, e.stderr)
        self.proc = subprocess.Popen(cmdlist, stdout=subprocess.PIPE, universal_newlines=True)
        return (_.strip() for _ in self.proc.stdout)

    class Error(subprocess.CalledProcessError): pass


def parselog(filelist, dirlist, stats, git, merge=False, filterlist=None):
    mtime = 0
    for line in git.log(merge, args.first_parent, args.commit_time, args.reverse_order,
            filterlist):
        stats['loglines'] += 1

        # Blank line between Date and list of files
        if not line: continue

        # File line
        if line[0] == ':':  # Faster than line.startswith(':')
            # If line describes a renaming, linetok has three tokens, otherwise two
            linetok = line.split('\t')
            status = linetok[0]
            file = linetok[-1]

            # Handles non-ASCII chars and OS path separator
            file = normalize(file)

            if file in filelist:
                stats['files'] -= 1
                log.debug("%d\t%d\t%d\t%s\t%s",
                          stats['loglines'], stats['commits'], stats['files'],
                          isodate(mtime), file)
                filelist.remove(file)
                try:
                    touch(os.path.join(git.workdir, file), mtime, args.test)
                    stats['touches'] += 1
                except Exception as e:
                    log.error("ERROR: %s", e)
                    stats['errors'] += 1

            if args.dirs:
                dirname = os.path.dirname(file)
                if status[-1] in ('A', 'D') and dirname in dirlist:
                    log.debug("%d\t%d\t-\t%s\t%s",
                              stats['loglines'], stats['commits'],
                              isodate(mtime), "{}/".format(dirname or '.'))
                    dirlist.remove(dirname)
                    try:
                        touch(os.path.join(git.workdir, dirname), mtime, args.test)
                        stats['dirtouches'] += 1
                    except Exception as e:
                        log.error("ERROR: %s", e)
                        stats['direrrors'] += 1

        # Date line
        else:
            stats['commits'] += 1
            mtime = int(line)

        # All files done?
        if not stats['files']:
            git.proc.terminate()  # hackish, but does the job. Not needed anyway
            return


# Main Logic ##################################################################

def main():
    start = time.time()  # yes, Wall time. CPU time is not realistic for users.
    stats = {_: 0 for _ in ('loglines', 'commits', 'touches', 'errors', 'dirtouches', 'direrrors')}

    # First things first: Where and Who are we?
    try:
        git = Git(args.workdir, args.gitdir)
    except Git.Error as e:
        # Not in a git repository, and git already informed user on stderr. So we just...
        return e.returncode

    # Do not work on dirty repositories, unless --force
    if not args.force and git.is_dirty():
        log.critical(
         "ERROR: There are local changes in the working directory.\n"
         "This could lead to undesirable results for modified files.\n"
         "Please, commit your changes (or use --force) and try again.\n"
         "Aborting")
        return 1

    # Get the files managed by git and build file and dir list to be processed
    filelist = set()
    dirlist  = set()
    if UPDATE_SYMLINKS and not args.skip_older_than:
        filelist = set(git.ls_files(args.pathspec))
        dirlist  = set(os.path.dirname(_) for _ in filelist)
    else:
        for path in git.ls_files(args.pathspec):
            fullpath = os.path.join(git.workdir, path)

            # Symlink (to file, to dir or broken - git handles the same way)
            if not UPDATE_SYMLINKS and os.path.islink(fullpath):
                log.warning("WARNING: Skipping symlink, OS does not support update: %s", path)
                continue

            # skip files which are older than given threshold
            if args.skip_older_than and start - os.path.getmtime(fullpath) > args.skip_older_than:
                continue

            # Always add them relative to worktree root
            filelist.add(path)
            dirlist.add(os.path.dirname(path))

    stats['totalfiles'] = stats['files'] = len(filelist)
    log.info("{0:,} files to be processed in work dir".format(stats['totalfiles']))

    if not filelist:
        # Nothing to do. Exit silently and without errors, just like git does
        return

    # Process the log until all files are 'touched'
    log.debug("Line #\tLog #\tF.Left\tModification Time\tFile Name")
    parselog(filelist, dirlist, stats, git, args.merge, args.pathspec)

    # Missing files
    if filelist:
        # Try to find them in merge logs, if not done already
        # (usually HUGE, thus MUCH slower!)
        if args.missing and not args.merge:
            filterlist = list(filelist)
            for i in range(0, len(filterlist), STEPMISSING):
                parselog(filelist, dirlist, stats, git,
                         merge=True, filterlist=filterlist[i:i+STEPMISSING])

        # Still missing some?
        for file in filelist:
            log.warning("WARNING: not found in the log: %s", file)

    # Final statistics
    # Suggestion: use git-log --before=mtime to brag about skipped log entries
    def loginfo(msg, *a, width=13):
        ifmt = '{:%d,}'    % (width,)  # not using 'n' for consistency with ffmt
        ffmt = '{:%d,.2f}' % (width,)
        # %-formatting lacks a thousand separator, must pre-render with .format()
        log.info(msg.replace('%d', ifmt).replace('%f', ffmt).format(*a))

    loginfo(
        "Statistics:\n"
        "%f seconds\n"
        "%d log lines processed\n"
        "%d commits evaluated",
        time.time()-start, stats['loglines'], stats['commits'])

    if args.dirs:
        if stats['direrrors']: loginfo("%d directory update errors", stats['direrrors'])
        loginfo("%d directories updated", stats['dirtouches'])

    if stats['touches'] != stats['totalfiles']: loginfo("%d files", stats['totalfiles'])
    if stats['files']:                          loginfo("%d files missing", stats['files'])
    if stats['errors']:                         loginfo("%d file update errors", stats['errors'])

    loginfo("%d files updated", stats['touches'])

    if args.test:
        log.info("TEST RUN - No files modified!")


args = parse_args()
log = setup_logging(args)
log.trace("Arguments: %s", args)

# UI done, it's show time!
try:
    sys.exit(main())
except KeyboardInterrupt:
    log.info("Aborting")
    sys.exit(-1)
