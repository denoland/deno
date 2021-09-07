# DO NOT MERGE IF THIS IS HERE
# Helper script to make iterating on this less painful with some new testdata.
# These new fixtures will replace the old ones.
rm -rf cov
mkdir cov

target/debug/deno test --coverage=cov $1
target/debug/deno coverage cov
