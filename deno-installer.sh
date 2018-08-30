#!/bin/bash

set -u

DENO_HOME="${HOME}/.deno"

main() {

  need_cmd downloader --check
  need_cmd uname
  need_cmd mktemp
  need_cmd chmod
  need_cmd mkdir
  need_cmd rm
  need_cmd rmdir

  get_architecture || return 1

  local _release_name=deno_"$RETVAL"_x64

  local _ext=".gz"
  case "$RETVAL" in
  *win*)
      _ext=".zip"
      ;;
  esac

  RELEASE_FILE_NAME="${_release_name}${_ext}"

  printf '%s\n' "Checking for latest release of deno for $RETVAL" 1>&2

  get_release_url $RELEASE_FILE_NAME

  local _dir="$(mktemp -d 2>/dev/null || ensure mktemp -d -t deno_installer)"
  local _file="${_dir}/${RELEASE_FILE_NAME}"

  printf '%s\n' "Downloading latest release of deno from $RELEASE_URL" 1>&2

  ensure downloader $RELEASE_URL $_file

  printf '%s\n' "Finished downloading. Deno will be installed in $DENO_HOME/bin" 1>&2

  local _deno_bin_dir="${DENO_HOME}/bin"

  ensure mkdir -p "$_deno_bin_dir"

  local _deno_exec="$_deno_bin_dir/deno"

  if [ $RETVAL = win ]; then
    unzip "$_file" > "$_deno_exec"
  else
    gunzip -c "$_file" > "$_deno_exec"
  fi

  ensure chmod u+x "$_deno_exec"

  printf '%s\n' "Add $DENO_HOME/bin to PATH variable in .bashrc for it to be availible globaly" 1>&2

  ignore rm "$_file"
  ignore rmdir "$_dir"

}

# This wraps curl or wget. Try curl first, if not installed,
# use wget instead.
downloader() {
    if check_cmd curl
    then _dld=curl
    elif check_cmd wget
    then _dld=wget
    else _dld='curl or wget' # to be used in error message of need_cmd
    fi

    if [ "$1" = --check ]
    then need_cmd "$_dld"
    elif [ "$_dld" = curl ]
    then curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]
    then wget "$1" -O "$2"
    else err "Unknown downloader"   # should not reach here
    fi
}

get_release_url() {
  # check if curl or wget is to be used
  downloader --check

  local _url=""

  # python code that will find the latest release for detected architecture
  local _get_latest_release_url='import sys, json; print [ str(asset["browser_download_url"]) for asset in json.load(sys.stdin)["assets"] if asset["name"] == "'$1'" ][0]'

  if [ "$_dld" = curl ]; then

    _url=$(curl -s https://api.github.com/repos/denoland/deno/releases/latest | python -c "$_get_latest_release_url")

  elif [ "$_dld" = wget ]; then

    ensure wget -q https://api.github.com/repos/denoland/deno/releases/latest
    _url=$(cat latest | python -c "$_get_latest_release_url")
    ensure rm latest

  else err "Unknown downloader"   # should not reach here

  fi

  RELEASE_URL=$_url
}


need_cmd() {
    if ! check_cmd "$1"
    then err "need '$1' (command not found)"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
    return $?
}

get_architecture() {

    local _ostype="$(uname -s)"

    case "$_ostype" in

        Linux)
            local _ostype=linux
            ;;

        Darwin)
            local _ostype=os_x
            ;;

        MINGW* | MSYS* | CYGWIN*)
            local _ostype=win
            ;;

        *)
            err "unrecognized OS type: $_ostype"
            ;;

    esac

    RETVAL="$_ostype"
}

need_ok() {
    if [ $? != 0 ]; then err "$1"; fi
}

assert_nz() {
    if [ -z "$1" ]; then err "assert_nz $2"; fi
}

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing
# command.
ensure() {
    "$@"
    need_ok "command failed: $*"
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
    "$@"
}

main "$@" || exit 1
