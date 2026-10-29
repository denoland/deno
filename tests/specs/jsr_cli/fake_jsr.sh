#!/bin/sh
# Stands in for the jsr binary: prints the arguments it was run with.
echo "jsr $*"
exit "${FAKE_JSR_EXIT_CODE:-0}"
