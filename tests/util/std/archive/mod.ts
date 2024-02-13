// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/*!
 * Ported and modified from: https://github.com/beatgammit/tar-js and
 * licensed as:
 *
 * (The MIT License)
 *
 * Copyright (c) 2011 T. Jameson Little
 * Copyright (c) 2019 Jun Kato
 * Copyright (c) 2018-2023 the Deno authors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

/**
 * Tar is a utility for collecting multiple files (or any arbitrary data) into one
 * archive file, while untar is the inverse utility to extract the files from an
 * archive.  Files are not compressed, only collected into the archive.
 *
 * ### File format and limitations
 *
 * The ustar file format is used for creating the archive file.
 * While this format is compatible with most tar readers,
 * the format has several limitations, including:
 * * Files must be smaller than 8GiB
 * * Filenames (including path) must be shorter than 256 characters
 * * Filenames (including path) cannot contain non-ASCII characters
 * * Sparse files are not supported
 * In addition to the ustar format, untar may also read from the pax format.
 * However, additional features, such as longer filenames, may be ignored.
 *
 * @module
 */
export * from "./tar.ts";
export * from "./untar.ts";
