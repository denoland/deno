#! /bin/sh

# error: missing field headers in $DENO_DIR/deps/**/*.metadata.json #16295

# https://github.com/denoland/deno/issues/16295

set -e
#set -x

gitdir=$(mktemp -d --suffix=-project)
git clone --depth 1 https://github.com/hayd/deno-udd $gitdir

mainScript=$gitdir/main.ts

export DENO_DIR=$(mktemp -d --suffix=-deno-dir)
echo "DENO_DIR: $DENO_DIR"
mkdir -p $DENO_DIR

alias deno=./target/debug/deno

# note: $DENO_DIR/lock.json is a non-standard location
deno cache --lock=$DENO_DIR/lock.json --lock-write ${mainScript}

# make deterministic DENO_DIR
find $DENO_DIR/deps -name '*.metadata.json' |
while read j
do
  # error: missing field `headers` at line 3 column 1
  cat $j | jq 'del(.headers) | del(.now)' >$j.new
  # workaround:
  #cat $j | jq '.headers={} | del(.now)' >$j.new
  mv $j.new $j

  # test: url is required
  echo '{}' >$j

  # TODO test: object is required
  #echo '' >$j
  #echo '""' >$j
  #echo '0' >$j
  #echo 'null' >$j
done
rm $DENO_DIR/dep_analysis_cache_v1



# this throws: error: missing field `headers`
# rebuild cache
(
  set -x
  deno cache --lock=$DENO_DIR/lock.json ${mainScript} || true
)



echo "hit enter to cleanup tempdirs: rm -rf $DENO_DIR $gitdir"
read

rm -rf $DENO_DIR $gitdir
