# Copyright 2014 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# This script should not be run directly but sourced by the other
# scripts (e.g. sysroot-creator-sid.sh).  Its up to the parent scripts
# to define certain environment variables: e.g.
#  DISTRO=debian
#  DIST=sid
#  # Similar in syntax to /etc/apt/sources.list
#  APT_SOURCES_LIST="http://ftp.us.debian.org/debian/ sid main"
#  KEYRING_FILE=debian-archive-sid-stable.gpg
#  DEBIAN_PACKAGES="gcc libz libssl"

#@ This script builds Debian/Ubuntu sysroot images for building Google Chrome.
#@
#@  Generally this script is invoked as:
#@  sysroot-creator-<flavour>.sh <mode> <args>*
#@  Available modes are shown below.
#@
#@ List of modes:

######################################################################
# Config
######################################################################

set -o nounset
set -o errexit

SCRIPT_DIR=$(cd $(dirname $0) && pwd)

if [ -z "${DIST:-}" ]; then
  echo "error: DIST not defined"
  exit 1
fi

if [ -z "${KEYRING_FILE:-}" ]; then
  echo "error: KEYRING_FILE not defined"
  exit 1
fi

if [ -z "${DEBIAN_PACKAGES:-}" ]; then
  echo "error: DEBIAN_PACKAGES not defined"
  exit 1
fi

readonly HAS_ARCH_AMD64=${HAS_ARCH_AMD64:=0}
readonly HAS_ARCH_I386=${HAS_ARCH_I386:=0}
readonly HAS_ARCH_ARM=${HAS_ARCH_ARM:=0}
readonly HAS_ARCH_ARM64=${HAS_ARCH_ARM64:=0}
readonly HAS_ARCH_MIPS=${HAS_ARCH_MIPS:=0}
readonly HAS_ARCH_MIPS64EL=${HAS_ARCH_MIPS64EL:=0}

readonly REQUIRED_TOOLS="curl xzcat"

######################################################################
# Package Config
######################################################################

readonly PACKAGES_EXT=xz
readonly RELEASE_FILE="Release"
readonly RELEASE_FILE_GPG="Release.gpg"

readonly DEBIAN_DEP_LIST_AMD64="packagelist.${DIST}.amd64"
readonly DEBIAN_DEP_LIST_I386="packagelist.${DIST}.i386"
readonly DEBIAN_DEP_LIST_ARM="packagelist.${DIST}.arm"
readonly DEBIAN_DEP_LIST_ARM64="packagelist.${DIST}.arm64"
readonly DEBIAN_DEP_LIST_MIPS="packagelist.${DIST}.mipsel"
readonly DEBIAN_DEP_LIST_MIPS64EL="packagelist.${DIST}.mips64el"


######################################################################
# Helper
######################################################################

Banner() {
  echo "######################################################################"
  echo $*
  echo "######################################################################"
}


SubBanner() {
  echo "----------------------------------------------------------------------"
  echo $*
  echo "----------------------------------------------------------------------"
}


Usage() {
  egrep "^#@" "${BASH_SOURCE[0]}" | cut --bytes=3-
}


DownloadOrCopy() {
  if [ -f "$2" ] ; then
    echo "$2 already in place"
    return
  fi

  HTTP=0
  echo "$1" | grep -qs ^http:// && HTTP=1
  if [ "$HTTP" = "1" ]; then
    SubBanner "downloading from $1 -> $2"
    # Appending the "$$" shell pid is necessary here to prevent concurrent
    # instances of sysroot-creator.sh from trying to write to the same file.
    # --create-dirs is added in case there are slashes in the filename, as can
    # happen with the "debian/security" release class.
    curl -L "$1" --create-dirs -o "${2}.partial.$$"
    mv "${2}.partial.$$" $2
  else
    SubBanner "copying from $1"
    cp "$1" "$2"
  fi
}


SetEnvironmentVariables() {
  ARCH=""
  echo $1 | grep -qs Amd64$ && ARCH=AMD64
  if [ -z "$ARCH" ]; then
    echo $1 | grep -qs I386$ && ARCH=I386
  fi
  if [ -z "$ARCH" ]; then
    echo $1 | grep -qs Mips64el$ && ARCH=MIPS64EL
  fi
  if [ -z "$ARCH" ]; then
    echo $1 | grep -qs Mips$ && ARCH=MIPS
  fi
  if [ -z "$ARCH" ]; then
    echo $1 | grep -qs ARM$ && ARCH=ARM
  fi
  if [ -z "$ARCH" ]; then
    echo $1 | grep -qs ARM64$ && ARCH=ARM64
  fi
  if [ -z "${ARCH}" ]; then
    echo "ERROR: Unable to determine architecture based on: $1"
    exit 1
  fi
  ARCH_LOWER=$(echo $ARCH | tr '[:upper:]' '[:lower:]')
}


# some sanity checks to make sure this script is run from the right place
# with the right tools
SanityCheck() {
  Banner "Sanity Checks"

  local chrome_dir=$(cd "${SCRIPT_DIR}/../../.." && pwd)
  BUILD_DIR="${chrome_dir}/out/sysroot-build/${DIST}"
  mkdir -p ${BUILD_DIR}
  echo "Using build directory: ${BUILD_DIR}"

  for tool in ${REQUIRED_TOOLS} ; do
    if ! which ${tool} > /dev/null ; then
      echo "Required binary $tool not found."
      echo "Exiting."
      exit 1
    fi
  done

  # This is where the staging sysroot is.
  INSTALL_ROOT="${BUILD_DIR}/${DIST}_${ARCH_LOWER}_staging"
  TARBALL="${BUILD_DIR}/${DISTRO}_${DIST}_${ARCH_LOWER}_sysroot.tar.xz"

  if ! mkdir -p "${INSTALL_ROOT}" ; then
    echo "ERROR: ${INSTALL_ROOT} can't be created."
    exit 1
  fi
}


ChangeDirectory() {
  # Change directory to where this script is.
  cd ${SCRIPT_DIR}
}


ClearInstallDir() {
  Banner "Clearing dirs in ${INSTALL_ROOT}"
  rm -rf ${INSTALL_ROOT}/*
}


CreateTarBall() {
  Banner "Creating tarball ${TARBALL}"
  tar -I "xz -9 -T0" -cf ${TARBALL} -C ${INSTALL_ROOT} .
}

ExtractPackageXz() {
  local src_file="$1"
  local dst_file="$2"
  local repo="$3"
  xzcat "${src_file}" | egrep '^(Package:|Filename:|SHA256:) ' |
    sed "s|Filename: |Filename: ${repo}|" > "${dst_file}"
}

GeneratePackageListDist() {
  local arch="$1"
  set -- $2
  local repo="$1"
  local dist="$2"
  local repo_name="$3"

  TMP_PACKAGE_LIST="${BUILD_DIR}/Packages.${dist}_${repo_name}_${arch}"
  local repo_basedir="${repo}/dists/${dist}"
  local package_list="${BUILD_DIR}/Packages.${dist}_${repo_name}_${arch}.${PACKAGES_EXT}"
  local package_file_arch="${repo_name}/binary-${arch}/Packages.${PACKAGES_EXT}"
  local package_list_arch="${repo_basedir}/${package_file_arch}"

  DownloadOrCopy "${package_list_arch}" "${package_list}"
  VerifyPackageListing "${package_file_arch}" "${package_list}" ${repo} ${dist}
  ExtractPackageXz "${package_list}" "${TMP_PACKAGE_LIST}" ${repo}
}

GeneratePackageListCommon() {
  local output_file="$1"
  local arch="$2"
  local packages="$3"

  local dists="${DIST} ${DIST_UPDATES:-}"
  local repos="main ${REPO_EXTRA:-}"

  local list_base="${BUILD_DIR}/Packages.${DIST}_${arch}"
  > "${list_base}"  # Create (or truncate) a zero-length file.
  echo "${APT_SOURCES_LIST}" | while read source; do
    GeneratePackageListDist "${arch}" "${source}"
    cat "${TMP_PACKAGE_LIST}" | ./merge-package-lists.py "${list_base}"
  done

  GeneratePackageList "${list_base}" "${output_file}" "${packages}"
}

GeneratePackageListAmd64() {
  GeneratePackageListCommon "$1" amd64 "${DEBIAN_PACKAGES}
    ${DEBIAN_PACKAGES_X86:=} ${DEBIAN_PACKAGES_AMD64:=}"
}

GeneratePackageListI386() {
  GeneratePackageListCommon "$1" i386 "${DEBIAN_PACKAGES}
    ${DEBIAN_PACKAGES_X86:=}"
}

GeneratePackageListARM() {
  GeneratePackageListCommon "$1" armhf "${DEBIAN_PACKAGES}
    ${DEBIAN_PACKAGES_ARM:=}"
}

GeneratePackageListARM64() {
  GeneratePackageListCommon "$1" arm64 "${DEBIAN_PACKAGES}
    ${DEBIAN_PACKAGES_ARM64:=}"
}

GeneratePackageListMips() {
  GeneratePackageListCommon "$1" mipsel "${DEBIAN_PACKAGES}"
}

GeneratePackageListMips64el() {
  GeneratePackageListCommon "$1" mips64el "${DEBIAN_PACKAGES}
  ${DEBIAN_PACKAGES_MIPS64EL:=}"
}

StripChecksumsFromPackageList() {
  local package_file="$1"
  sed -i 's/ [a-f0-9]\{64\}$//' "$package_file"
}

VerifyPackageFilesMatch() {
  local downloaded_package_file="$1"
  local stored_package_file="$2"
  diff -u "$downloaded_package_file" "$stored_package_file"
  if [ "$?" -ne "0" ]; then
    echo "ERROR: downloaded package files does not match $2."
    echo "You may need to run UpdatePackageLists."
    exit 1
  fi
}

######################################################################
#
######################################################################

HacksAndPatchesCommon() {
  local arch=$1
  local os=$2
  local strip=$3
  Banner "Misc Hacks & Patches"
  # these are linker scripts with absolute pathnames in them
  # which we rewrite here
  lscripts="${INSTALL_ROOT}/usr/lib/${arch}-${os}/libpthread.so \
            ${INSTALL_ROOT}/usr/lib/${arch}-${os}/libc.so"

  # Rewrite linker scripts
  sed -i -e 's|/usr/lib/${arch}-${os}/||g'  ${lscripts}
  sed -i -e 's|/lib/${arch}-${os}/||g' ${lscripts}

  # Unversion libdbus symbols.  This is required because libdbus-1-3
  # switched from unversioned symbols to versioned ones, and we must
  # still support distros using the unversioned library.  This hack
  # can be removed once support for Ubuntu Trusty and Debian Jessie
  # are dropped.
  ${strip} -R .gnu.version_d -R .gnu.version \
    "${INSTALL_ROOT}/lib/${arch}-${os}/libdbus-1.so.3"
  cp "${SCRIPT_DIR}/libdbus-1-3-symbols" \
    "${INSTALL_ROOT}/debian/libdbus-1-3/DEBIAN/symbols"

  # Glibc 2.27 introduced some new optimizations to several math functions, but
  # it will be a while before it makes it into all supported distros.  Luckily,
  # glibc maintains ABI compatibility with previous versions, so the old symbols
  # are still there.
  # TODO(thomasanderson): Remove this once glibc 2.27 is available on all
  # supported distros.
  local math_h="${INSTALL_ROOT}/usr/include/math.h"
  local libm_so="${INSTALL_ROOT}/lib/${arch}-${os}/libm.so.6"
  nm -D --defined-only --with-symbol-versions "${libm_so}" | \
    "${SCRIPT_DIR}/find_incompatible_glibc_symbols.py" >> "${math_h}"

  # glob64() was also optimized in glibc 2.27.  Make sure to choose the older
  # version.
  local glob_h="${INSTALL_ROOT}/usr/include/glob.h"
  local libc_so="${INSTALL_ROOT}/lib/${arch}-${os}/libc.so.6"
  nm -D --defined-only --with-symbol-versions "${libc_so}" | \
    "${SCRIPT_DIR}/find_incompatible_glibc_symbols.py" >> "${glob_h}"

  # This is for chrome's ./build/linux/pkg-config-wrapper
  # which overwrites PKG_CONFIG_LIBDIR internally
  SubBanner "Move pkgconfig scripts"
  mkdir -p ${INSTALL_ROOT}/usr/lib/pkgconfig
  mv ${INSTALL_ROOT}/usr/lib/${arch}-${os}/pkgconfig/* \
      ${INSTALL_ROOT}/usr/lib/pkgconfig
}


HacksAndPatchesAmd64() {
  HacksAndPatchesCommon x86_64 linux-gnu strip
}


HacksAndPatchesI386() {
  HacksAndPatchesCommon i386 linux-gnu strip
}


HacksAndPatchesARM() {
  HacksAndPatchesCommon arm linux-gnueabihf arm-linux-gnueabihf-strip
}


HacksAndPatchesARM64() {
  # Use the unstripped libdbus for arm64 to prevent linker errors.
  # https://bugs.chromium.org/p/webrtc/issues/detail?id=8535
  HacksAndPatchesCommon aarch64 linux-gnu true
}


HacksAndPatchesMips() {
  HacksAndPatchesCommon mipsel linux-gnu mipsel-linux-gnu-strip
}


HacksAndPatchesMips64el() {
  HacksAndPatchesCommon mips64el linux-gnuabi64 mips64el-linux-gnuabi64-strip
}


InstallIntoSysroot() {
  Banner "Install Libs And Headers Into Jail"

  mkdir -p ${BUILD_DIR}/debian-packages
  # The /debian directory is an implementation detail that's used to cd into
  # when running dpkg-shlibdeps.
  mkdir -p ${INSTALL_ROOT}/debian
  # An empty control file is necessary to run dpkg-shlibdeps.
  touch ${INSTALL_ROOT}/debian/control
  while (( "$#" )); do
    local file="$1"
    local package="${BUILD_DIR}/debian-packages/${file##*/}"
    shift
    local sha256sum="$1"
    shift
    if [ "${#sha256sum}" -ne "64" ]; then
      echo "Bad sha256sum from package list"
      exit 1
    fi

    Banner "Installing $(basename ${file})"
    DownloadOrCopy ${file} ${package}
    if [ ! -s "${package}" ] ; then
      echo
      echo "ERROR: bad package ${package}"
      exit 1
    fi
    echo "${sha256sum}  ${package}" | sha256sum --quiet -c

    SubBanner "Extracting to ${INSTALL_ROOT}"
    dpkg-deb -x ${package} ${INSTALL_ROOT}

    base_package=$(dpkg-deb --field ${package} Package)
    mkdir -p ${INSTALL_ROOT}/debian/${base_package}/DEBIAN
    dpkg-deb -e ${package} ${INSTALL_ROOT}/debian/${base_package}/DEBIAN
  done

  # Prune /usr/share, leaving only pkgconfig
  for name in ${INSTALL_ROOT}/usr/share/*; do
    if [ "${name}" != "${INSTALL_ROOT}/usr/share/pkgconfig" ]; then
      rm -r ${name}
    fi
  done
}


CleanupJailSymlinks() {
  Banner "Jail symlink cleanup"

  SAVEDPWD=$(pwd)
  cd ${INSTALL_ROOT}
  local libdirs="lib usr/lib"
  if [ "${ARCH}" != "MIPS" ]; then
    libdirs="${libdirs} lib64"
  fi
  find $libdirs -type l -printf '%p %l\n' | while read link target; do
    # skip links with non-absolute paths
    echo "${target}" | grep -qs ^/ || continue
    echo "${link}: ${target}"
    # Relativize the symlink.
    prefix=$(echo "${link}" | sed -e 's/[^/]//g' | sed -e 's|/|../|g')
    ln -snfv "${prefix}${target}" "${link}"
  done

  find $libdirs -type l -printf '%p %l\n' | while read link target; do
    # Make sure we catch new bad links.
    if [ ! -r "${link}" ]; then
      echo "ERROR: FOUND BAD LINK ${link}"
      ls -l ${link}
      exit 1
    fi
  done
  cd "$SAVEDPWD"
}


VerifyLibraryDepsCommon() {
  local arch=$1
  local os=$2
  local find_dirs=(
    "${INSTALL_ROOT}/lib/${arch}-${os}/"
    "${INSTALL_ROOT}/usr/lib/${arch}-${os}/"
  )
  local needed_libs="$(
    find ${find_dirs[*]} -name "*\.so*" -type f -exec file {} \; | \
      grep ': ELF' | sed 's/^\(.*\): .*$/\1/' | xargs readelf -d | \
      grep NEEDED | sort | uniq | sed 's/^.*Shared library: \[\(.*\)\]$/\1/g')"
  local all_libs="$(find ${find_dirs[*]} -printf '%f\n')"
  local missing_libs="$(grep -vFxf <(echo "${all_libs}") \
    <(echo "${needed_libs}"))"
  if [ ! -z "${missing_libs}" ]; then
    echo "Missing libraries:"
    echo "${missing_libs}"
    exit 1
  fi
}


VerifyLibraryDepsAmd64() {
  VerifyLibraryDepsCommon x86_64 linux-gnu
}


VerifyLibraryDepsI386() {
  VerifyLibraryDepsCommon i386 linux-gnu
}


VerifyLibraryDepsARM() {
  VerifyLibraryDepsCommon arm linux-gnueabihf
}


VerifyLibraryDepsARM64() {
  VerifyLibraryDepsCommon aarch64 linux-gnu
}


VerifyLibraryDepsMips() {
  VerifyLibraryDepsCommon mipsel linux-gnu
}


VerifyLibraryDepsMips64el() {
  VerifyLibraryDepsCommon mips64el linux-gnuabi64
}


#@
#@ BuildSysrootAmd64
#@
#@    Build everything and package it
BuildSysrootAmd64() {
  if [ "$HAS_ARCH_AMD64" = "0" ]; then
    return
  fi
  ClearInstallDir
  local package_file="$BUILD_DIR/package_with_sha256sum_amd64"
  GeneratePackageListAmd64 "$package_file"
  local files_and_sha256sums="$(cat ${package_file})"
  StripChecksumsFromPackageList "$package_file"
  VerifyPackageFilesMatch "$package_file" "$DEBIAN_DEP_LIST_AMD64"
  InstallIntoSysroot ${files_and_sha256sums}
  CleanupJailSymlinks
  HacksAndPatchesAmd64
  VerifyLibraryDepsAmd64
  CreateTarBall
}

#@
#@ BuildSysrootI386
#@
#@    Build everything and package it
BuildSysrootI386() {
  if [ "$HAS_ARCH_I386" = "0" ]; then
    return
  fi
  ClearInstallDir
  local package_file="$BUILD_DIR/package_with_sha256sum_i386"
  GeneratePackageListI386 "$package_file"
  local files_and_sha256sums="$(cat ${package_file})"
  StripChecksumsFromPackageList "$package_file"
  VerifyPackageFilesMatch "$package_file" "$DEBIAN_DEP_LIST_I386"
  InstallIntoSysroot ${files_and_sha256sums}
  CleanupJailSymlinks
  HacksAndPatchesI386
  VerifyLibraryDepsI386
  CreateTarBall
}

#@
#@ BuildSysrootARM
#@
#@    Build everything and package it
BuildSysrootARM() {
  if [ "$HAS_ARCH_ARM" = "0" ]; then
    return
  fi
  ClearInstallDir
  local package_file="$BUILD_DIR/package_with_sha256sum_arm"
  GeneratePackageListARM "$package_file"
  local files_and_sha256sums="$(cat ${package_file})"
  StripChecksumsFromPackageList "$package_file"
  VerifyPackageFilesMatch "$package_file" "$DEBIAN_DEP_LIST_ARM"
  InstallIntoSysroot ${files_and_sha256sums}
  CleanupJailSymlinks
  HacksAndPatchesARM
  VerifyLibraryDepsARM
  CreateTarBall
}

#@
#@ BuildSysrootARM64
#@
#@    Build everything and package it
BuildSysrootARM64() {
  if [ "$HAS_ARCH_ARM64" = "0" ]; then
    return
  fi
  ClearInstallDir
  local package_file="$BUILD_DIR/package_with_sha256sum_arm64"
  GeneratePackageListARM64 "$package_file"
  local files_and_sha256sums="$(cat ${package_file})"
  StripChecksumsFromPackageList "$package_file"
  VerifyPackageFilesMatch "$package_file" "$DEBIAN_DEP_LIST_ARM64"
  InstallIntoSysroot ${files_and_sha256sums}
  CleanupJailSymlinks
  HacksAndPatchesARM64
  VerifyLibraryDepsARM64
  CreateTarBall
}


#@
#@ BuildSysrootMips
#@
#@    Build everything and package it
BuildSysrootMips() {
  if [ "$HAS_ARCH_MIPS" = "0" ]; then
    return
  fi
  ClearInstallDir
  local package_file="$BUILD_DIR/package_with_sha256sum_mips"
  GeneratePackageListMips "$package_file"
  local files_and_sha256sums="$(cat ${package_file})"
  StripChecksumsFromPackageList "$package_file"
  VerifyPackageFilesMatch "$package_file" "$DEBIAN_DEP_LIST_MIPS"
  InstallIntoSysroot ${files_and_sha256sums}
  CleanupJailSymlinks
  HacksAndPatchesMips
  VerifyLibraryDepsMips
  CreateTarBall
}


#@
#@ BuildSysrootMips64el
#@
#@    Build everything and package it
BuildSysrootMips64el() {
  if [ "$HAS_ARCH_MIPS64EL" = "0" ]; then
    return
  fi
  ClearInstallDir
  local package_file="$BUILD_DIR/package_with_sha256sum_mips64el"
  GeneratePackageListMips64el "$package_file"
  local files_and_sha256sums="$(cat ${package_file})"
  StripChecksumsFromPackageList "$package_file"
  VerifyPackageFilesMatch "$package_file" "$DEBIAN_DEP_LIST_MIPS64EL"
  InstallIntoSysroot ${files_and_sha256sums}
  CleanupJailSymlinks
  HacksAndPatchesMips64el
  VerifyLibraryDepsMips64el
  CreateTarBall
}

#@
#@ BuildSysrootAll
#@
#@    Build sysroot images for all architectures
BuildSysrootAll() {
  RunCommand BuildSysrootAmd64
  RunCommand BuildSysrootI386
  RunCommand BuildSysrootARM
  RunCommand BuildSysrootARM64
  RunCommand BuildSysrootMips
  RunCommand BuildSysrootMips64el
}

UploadSysroot() {
  local rev=$1
  if [ -z "${rev}" ]; then
    echo "Please specify a revision to upload at."
    exit 1
  fi
  set -x
  gsutil cp -a public-read "${TARBALL}" \
      "gs://chrome-linux-sysroot/toolchain/$rev/"
  set +x
}

#@
#@ UploadSysrootAmd64 <revision>
#@
UploadSysrootAmd64() {
  if [ "$HAS_ARCH_AMD64" = "0" ]; then
    return
  fi
  UploadSysroot "$@"
}

#@
#@ UploadSysrootI386 <revision>
#@
UploadSysrootI386() {
  if [ "$HAS_ARCH_I386" = "0" ]; then
    return
  fi
  UploadSysroot "$@"
}

#@
#@ UploadSysrootARM <revision>
#@
UploadSysrootARM() {
  if [ "$HAS_ARCH_ARM" = "0" ]; then
    return
  fi
  UploadSysroot "$@"
}

#@
#@ UploadSysrootARM64 <revision>
#@
UploadSysrootARM64() {
  if [ "$HAS_ARCH_ARM64" = "0" ]; then
    return
  fi
  UploadSysroot "$@"
}

#@
#@ UploadSysrootMips <revision>
#@
UploadSysrootMips() {
  if [ "$HAS_ARCH_MIPS" = "0" ]; then
    return
  fi
  UploadSysroot "$@"
}

#@
#@ UploadSysrootMips64el <revision>
#@
UploadSysrootMips64el() {
  if [ "$HAS_ARCH_MIPS64EL" = "0" ]; then
    return
  fi
  UploadSysroot "$@"
}

#@
#@ UploadSysrootAll <revision>
#@
#@    Upload sysroot image for all architectures
UploadSysrootAll() {
  RunCommand UploadSysrootAmd64 "$@"
  RunCommand UploadSysrootI386 "$@"
  RunCommand UploadSysrootARM "$@"
  RunCommand UploadSysrootARM64 "$@"
  RunCommand UploadSysrootMips "$@"
  RunCommand UploadSysrootMips64el "$@"

}

#
# CheckForDebianGPGKeyring
#
#     Make sure the Debian GPG keys exist. Otherwise print a helpful message.
#
CheckForDebianGPGKeyring() {
  if [ ! -e "$KEYRING_FILE" ]; then
    echo "KEYRING_FILE not found: ${KEYRING_FILE}"
    echo "Debian GPG keys missing. Install the debian-archive-keyring package."
    exit 1
  fi
}

#
# VerifyPackageListing
#
#     Verifies the downloaded Packages.xz file has the right checksums.
#
VerifyPackageListing() {
  local file_path="$1"
  local output_file="$2"
  local repo="$3"
  local dist="$4"

  local repo_basedir="${repo}/dists/${dist}"
  local release_list="${repo_basedir}/${RELEASE_FILE}"
  local release_list_gpg="${repo_basedir}/${RELEASE_FILE_GPG}"

  local release_file="${BUILD_DIR}/${dist}-${RELEASE_FILE}"
  local release_file_gpg="${BUILD_DIR}/${dist}-${RELEASE_FILE_GPG}"

  CheckForDebianGPGKeyring

  DownloadOrCopy ${release_list} ${release_file}
  DownloadOrCopy ${release_list_gpg} ${release_file_gpg}
  echo "Verifying: ${release_file} with ${release_file_gpg}"
  set -x
  gpgv --keyring "${KEYRING_FILE}" "${release_file_gpg}" "${release_file}"
  set +x

  echo "Verifying: ${output_file}"
  local sha256sum=$(grep -E "${file_path}\$|:\$" "${release_file}" | \
    grep "SHA256:" -A 1 | xargs echo | awk '{print $2;}')

  if [ "${#sha256sum}" -ne "64" ]; then
    echo "Bad sha256sum from ${release_list}"
    exit 1
  fi

  echo "${sha256sum}  ${output_file}" | sha256sum --quiet -c
}

#
# GeneratePackageList
#
#     Looks up package names in ${BUILD_DIR}/Packages and write list of URLs
#     to output file.
#
GeneratePackageList() {
  local input_file="$1"
  local output_file="$2"
  echo "Updating: ${output_file} from ${input_file}"
  /bin/rm -f "${output_file}"
  shift
  shift
  for pkg in $@ ; do
    local pkg_full=$(grep -A 1 " ${pkg}\$" "$input_file" | \
      egrep "pool/.*" | sed 's/.*Filename: //')
    if [ -z "${pkg_full}" ]; then
        echo "ERROR: missing package: $pkg"
        exit 1
    fi
    local sha256sum=$(grep -A 4 " ${pkg}\$" "$input_file" | \
      grep ^SHA256: | sed 's/^SHA256: //')
    if [ "${#sha256sum}" -ne "64" ]; then
      echo "Bad sha256sum from Packages"
      exit 1
    fi
    echo $pkg_full $sha256sum >> "$output_file"
  done
  # sort -o does an in-place sort of this file
  sort "$output_file" -o "$output_file"
}

#@
#@ UpdatePackageListsAmd64
#@
#@     Regenerate the package lists such that they contain an up-to-date
#@     list of URLs within the Debian archive. (For amd64)
UpdatePackageListsAmd64() {
  if [ "$HAS_ARCH_AMD64" = "0" ]; then
    return
  fi
  GeneratePackageListAmd64 "$DEBIAN_DEP_LIST_AMD64"
  StripChecksumsFromPackageList "$DEBIAN_DEP_LIST_AMD64"
}

#@
#@ UpdatePackageListsI386
#@
#@     Regenerate the package lists such that they contain an up-to-date
#@     list of URLs within the Debian archive. (For i386)
UpdatePackageListsI386() {
  if [ "$HAS_ARCH_I386" = "0" ]; then
    return
  fi
  GeneratePackageListI386 "$DEBIAN_DEP_LIST_I386"
  StripChecksumsFromPackageList "$DEBIAN_DEP_LIST_I386"
}

#@
#@ UpdatePackageListsARM
#@
#@     Regenerate the package lists such that they contain an up-to-date
#@     list of URLs within the Debian archive. (For arm)
UpdatePackageListsARM() {
  if [ "$HAS_ARCH_ARM" = "0" ]; then
    return
  fi
  GeneratePackageListARM "$DEBIAN_DEP_LIST_ARM"
  StripChecksumsFromPackageList "$DEBIAN_DEP_LIST_ARM"
}

#@
#@ UpdatePackageListsARM64
#@
#@     Regenerate the package lists such that they contain an up-to-date
#@     list of URLs within the Debian archive. (For arm64)
UpdatePackageListsARM64() {
  if [ "$HAS_ARCH_ARM64" = "0" ]; then
    return
  fi
  GeneratePackageListARM64 "$DEBIAN_DEP_LIST_ARM64"
  StripChecksumsFromPackageList "$DEBIAN_DEP_LIST_ARM64"
}

#@
#@ UpdatePackageListsMips
#@
#@     Regenerate the package lists such that they contain an up-to-date
#@     list of URLs within the Debian archive. (For mips)
UpdatePackageListsMips() {
  if [ "$HAS_ARCH_MIPS" = "0" ]; then
    return
  fi
  GeneratePackageListMips "$DEBIAN_DEP_LIST_MIPS"
  StripChecksumsFromPackageList "$DEBIAN_DEP_LIST_MIPS"
}


#@
#@ UpdatePackageListsMips64el
#@
#@     Regenerate the package lists such that they contain an up-to-date
#@     list of URLs within the Debian archive. (For mips64el)
UpdatePackageListsMips64el() {
  if [ "$HAS_ARCH_MIPS64EL" = "0" ]; then
    return
  fi
  GeneratePackageListMips64el "$DEBIAN_DEP_LIST_MIPS64EL"
  StripChecksumsFromPackageList "$DEBIAN_DEP_LIST_MIPS64EL"
}

#@
#@ UpdatePackageListsAll
#@
#@    Regenerate the package lists for all architectures.
UpdatePackageListsAll() {
  RunCommand UpdatePackageListsAmd64
  RunCommand UpdatePackageListsI386
  RunCommand UpdatePackageListsARM
  RunCommand UpdatePackageListsARM64
  RunCommand UpdatePackageListsMips
  RunCommand UpdatePackageListsMips64el
}

#@
#@ PrintArchitectures
#@
#@    Prints supported architectures.
PrintArchitectures() {
  if [ "$HAS_ARCH_AMD64" = "1" ]; then
    echo Amd64
  fi
  if [ "$HAS_ARCH_I386" = "1" ]; then
    echo I386
  fi
  if [ "$HAS_ARCH_ARM" = "1" ]; then
    echo ARM
  fi
  if [ "$HAS_ARCH_ARM64" = "1" ]; then
    echo ARM64
  fi
  if [ "$HAS_ARCH_MIPS" = "1" ]; then
    echo Mips
  fi
  if [ "$HAS_ARCH_MIPS64EL" = "1" ]; then
    echo Mips64el
  fi
}

#@
#@ PrintDistro
#@
#@    Prints distro.  eg: ubuntu
PrintDistro() {
  echo ${DISTRO}
}

#@
#@ DumpRelease
#@
#@    Prints disto release.  eg: jessie
PrintRelease() {
  echo ${DIST}
}

RunCommand() {
  SetEnvironmentVariables "$1"
  SanityCheck
  "$@"
}

if [ $# -eq 0 ] ; then
  echo "ERROR: you must specify a mode on the commandline"
  echo
  Usage
  exit 1
elif [ "$(type -t $1)" != "function" ]; then
  echo "ERROR: unknown function '$1'." >&2
  echo "For help, try:"
  echo "    $0 help"
  exit 1
else
  ChangeDirectory
  if echo $1 | grep -qs --regexp='\(^Print\)\|\(All$\)'; then
    "$@"
  else
    RunCommand "$@"
  fi
fi
