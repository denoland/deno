// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

//File access constants
export const F_OK = 0;
export const R_OK = 4;
export const W_OK = 2;
export const X_OK = 1;

//File mode constants
export const S_IRUSR = 0o400; //read by owner
export const S_IWUSR = 0o200; //write by owner
export const S_IXUSR = 0o100; //execute/search by owner
export const S_IRGRP = 0o40; //read by group
export const S_IWGRP = 0o20; //write by group
export const S_IXGRP = 0o10; //execute/search by group
export const S_IROTH = 0o4; //read by others
export const S_IWOTH = 0o2; //write by others
export const S_IXOTH = 0o1; //execute/search by others
