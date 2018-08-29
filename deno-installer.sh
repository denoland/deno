#!/bin/bash

set -u

main() {

  need_cmd downloader --check
  need_cmd uname
  need_cmd mktemp
  need_cmd chmod
  need_cmd mkdir
  need_cmd rm
  need_cmd rmdir
  need_cmd jq

  get_architecture || return 1

  local _release_name=deno_"$RETVAL"_x64

  get_release_url "$_release_name"

  local _ext=".gz"
  case "$RETVAL" in
  *win*)
      _ext=".zip"
      ;;
  esac

  local _dir="$(mktemp -d 2>/dev/null || ensure mktemp -d -t deno_installer)"
  local _file="${_dir}/deno_${RETVAL}_x64${_ext}"

  ensure downloader $RELEASE_URL $_file

  local _deno_home="${HOME}/deno"
  local _deno_bin="$_deno_home/deno"

  ensure mkdir -p "$_deno_home"

  gunzip -c "$_file" > "$_deno_bin"

  ensure chmod u+x "$_deno_bin"

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

# the only way I found to get the URL to the latest release from github
# unfortunatelly it required `jq` to parse JSON output
get_release_url() {
  local _url=""
  # TODO cURL or wget condition
  _url=$(curl -s https://api.github.com/repos/denoland/deno/releases/latest | jq -r ".assets[] | select(.name | test(\"$1\")) | .browser_download_url")

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
