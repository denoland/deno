#! /bin/sh

# feat: deno cache: add option: deterministic

# https://github.com/denoland/deno/issues/16296

# cargo build && ./repro.sh

set -e
#set -x

#export RUST_BACKTRACE=1
#export RUST_BACKTRACE=all
export RUST_BACKTRACE=full

gitdir=$(mktemp -d --suffix=-project)
git clone --depth 1 https://github.com/hayd/deno-udd $gitdir

logdir=$(mktemp -d --suffix=-log)

mainScript=$gitdir/main.ts

export DENO_DIR=$(mktemp -d --suffix=-deno-dir)
echo "DENO_DIR: $DENO_DIR"
mkdir -p $DENO_DIR

alias deno=./target/debug/deno

if false
then
  # old version
  # call "deno cache" without "--deterministic"
  # note: $DENO_DIR/lock.json is a non-standard location
  deno cache --lock=$DENO_DIR/lock.json --lock-write ${mainScript}

  # this workaround is needed without "--deterministic"

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

  # rebuild dep_analysis_cache_v1
  deno cache --lock=$DENO_DIR/lock.json ${mainScript}

else
  # new version
  # call "deno cache" with "--deterministic"

  # note: $DENO_DIR/lock.json is a non-standard location
  deno cache --lock=$DENO_DIR/lock.json --lock-write ${mainScript} --deterministic 2>&1 |
  tee $logdir/deno-cache.log
fi



echo "testing if metadata is deterministic"

file=$(find $DENO_DIR/deps -name '*.metadata.json' | head -n1)

echo "testing metadata file: $file"
cat $file
echo

for key in headers now
do
  val="$(cat $file | jq -r ".$key")"
  if [[ "$val" == "null" ]]
  then
    echo "pass: metadata.$key is missing"
  else
    echo "fail: metadata.$key is present:"
    echo "$val"
  fi
done



echo "logs:"
echo "  $logdir/deno-cache.log"



echo "hit enter to cleanup tempdirs:"
echo "  rm -rf $DENO_DIR $gitdir $logdir"
read

rm -rf $DENO_DIR $gitdir $logdir
