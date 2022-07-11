cmd_Release/sqlite3.a := rm -f Release/sqlite3.a && ./gyp-mac-tool filter-libtool libtool  -static -o Release/sqlite3.a Release/obj.target/sqlite3/gen/sqlite3/sqlite3.o
