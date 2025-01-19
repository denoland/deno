ARCH = $(shell uname -m)

liblzld_${ARCH}.a: lzld.m
	cc -c lzld.m -o lzld.o
	ar rcs liblzld_${ARCH}.a lzld.o

clean:
	rm -f liblzld_${ARCH}.a lzld.o

all: liblzld_${ARCH}.a

.PHONY: clean all
