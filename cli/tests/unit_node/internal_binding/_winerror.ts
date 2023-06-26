// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/*++

Copyright (c) Microsoft Corporation. All rights reserved.

You may only use this code if you agree to the terms of the Windows Research Kernel Source Code License agreement (see License.txt).
If you do not agree to the terms, do not use the code.

Module:

    winderror.h

Abstract:

    Win32 API functions

--*/

// This module ports:
// - https://raw.githubusercontent.com/mic101/windows/master/WRK-v1.2/public/sdk/inc/winerror.h

// MessageId: ERROR_SUCCESS
//
// MessageText:
//
//  The operation completed successfully.
//
export const ERROR_SUCCESS = 0;

//
// MessageId: ERROR_INVALID_FUNCTION
//
// MessageText:
//
//  Incorrect function.
//
export const ERROR_INVALID_FUNCTION = 1; // dderror

//
// MessageId: ERROR_FILE_NOT_FOUND
//
// MessageText:
//
//  The system cannot find the file specified.
//
export const ERROR_FILE_NOT_FOUND = 2;

//
// MessageId: ERROR_PATH_NOT_FOUND
//
// MessageText:
//
//  The system cannot find the path specified.
//
export const ERROR_PATH_NOT_FOUND = 3;

//
// MessageId: ERROR_TOO_MANY_OPEN_FILES
//
// MessageText:
//
//  The system cannot open the file.
//
export const ERROR_TOO_MANY_OPEN_FILES = 4;

//
// MessageId: ERROR_ACCESS_DENIED
//
// MessageText:
//
//  Access is denied.
//
export const ERROR_ACCESS_DENIED = 5;

//
// MessageId: ERROR_INVALID_HANDLE
//
// MessageText:
//
//  The handle is invalid.
//
export const ERROR_INVALID_HANDLE = 6;

//
// MessageId: ERROR_ARENA_TRASHED
//
// MessageText:
//
//  The storage control blocks were destroyed.
//
export const ERROR_ARENA_TRASHED = 7;

//
// MessageId: ERROR_NOT_ENOUGH_MEMORY
//
// MessageText:
//
//  Not enough storage is available to process this command.
//
export const ERROR_NOT_ENOUGH_MEMORY = 8; // dderror

//
// MessageId: ERROR_INVALID_BLOCK
//
// MessageText:
//
//  The storage control block address is invalid.
//
export const ERROR_INVALID_BLOCK = 9;

//
// MessageId: ERROR_BAD_ENVIRONMENT
//
// MessageText:
//
//  The environment is incorrect.
//
export const ERROR_BAD_ENVIRONMENT = 10;

//
// MessageId: ERROR_BAD_FORMAT
//
// MessageText:
//
//  An attempt was made to load a program with an incorrect format.
//
export const ERROR_BAD_FORMAT = 11;

//
// MessageId: ERROR_INVALID_ACCESS
//
// MessageText:
//
//  The access code is invalid.
//
export const ERROR_INVALID_ACCESS = 12;

//
// MessageId: ERROR_INVALID_DATA
//
// MessageText:
//
//  The data is invalid.
//
export const ERROR_INVALID_DATA = 13;

//
// MessageId: ERROR_OUTOFMEMORY
//
// MessageText:
//
//  Not enough storage is available to complete this operation.
//
export const ERROR_OUTOFMEMORY = 14;

//
// MessageId: ERROR_INVALID_DRIVE
//
// MessageText:
//
//  The system cannot find the drive specified.
//
export const ERROR_INVALID_DRIVE = 15;

//
// MessageId: ERROR_CURRENT_DIRECTORY
//
// MessageText:
//
//  The directory cannot be removed.
//
export const ERROR_CURRENT_DIRECTORY = 16;

//
// MessageId: ERROR_NOT_SAME_DEVICE
//
// MessageText:
//
//  The system cannot move the file to a different disk drive.
//
export const ERROR_NOT_SAME_DEVICE = 17;

//
// MessageId: ERROR_NO_MORE_FILES
//
// MessageText:
//
//  There are no more files.
//
export const ERROR_NO_MORE_FILES = 18;

//
// MessageId: ERROR_WRITE_PROTECT
//
// MessageText:
//
//  The media is write protected.
//
export const ERROR_WRITE_PROTECT = 19;

//
// MessageId: ERROR_BAD_UNIT
//
// MessageText:
//
//  The system cannot find the device specified.
//
export const ERROR_BAD_UNIT = 20;

//
// MessageId: ERROR_NOT_READY
//
// MessageText:
//
//  The device is not ready.
//
export const ERROR_NOT_READY = 21;

//
// MessageId: ERROR_BAD_COMMAND
//
// MessageText:
//
//  The device does not recognize the command.
//
export const ERROR_BAD_COMMAND = 22;

//
// MessageId: ERROR_CRC
//
// MessageText:
//
//  Data error (cyclic redundancy check).
//
export const ERROR_CRC = 23;

//
// MessageId: ERROR_BAD_LENGTH
//
// MessageText:
//
//  The program issued a command but the command length is incorrect.
//
export const ERROR_BAD_LENGTH = 24;

//
// MessageId: ERROR_SEEK
//
// MessageText:
//
//  The drive cannot locate a specific area or track on the disk.
//
export const ERROR_SEEK = 25;

//
// MessageId: ERROR_NOT_DOS_DISK
//
// MessageText:
//
//  The specified disk or diskette cannot be accessed.
//
export const ERROR_NOT_DOS_DISK = 26;

//
// MessageId: ERROR_SECTOR_NOT_FOUND
//
// MessageText:
//
//  The drive cannot find the sector requested.
//
export const ERROR_SECTOR_NOT_FOUND = 27;

//
// MessageId: ERROR_OUT_OF_PAPER
//
// MessageText:
//
//  The printer is out of paper.
//
export const ERROR_OUT_OF_PAPER = 28;

//
// MessageId: ERROR_WRITE_FAULT
//
// MessageText:
//
//  The system cannot write to the specified device.
//
export const ERROR_WRITE_FAULT = 29;

//
// MessageId: ERROR_READ_FAULT
//
// MessageText:
//
//  The system cannot read from the specified device.
//
export const ERROR_READ_FAULT = 30;

//
// MessageId: ERROR_GEN_FAILURE
//
// MessageText:
//
//  A device attached to the system is not functioning.
//
export const ERROR_GEN_FAILURE = 31;

//
// MessageId: ERROR_SHARING_VIOLATION
//
// MessageText:
//
//  The process cannot access the file because it is being used by another process.
//
export const ERROR_SHARING_VIOLATION = 32;

//
// MessageId: ERROR_LOCK_VIOLATION
//
// MessageText:
//
//  The process cannot access the file because another process has locked a portion of the file.
//
export const ERROR_LOCK_VIOLATION = 33;

//
// MessageId: ERROR_WRONG_DISK
//
// MessageText:
//
//  The wrong diskette is in the drive.
//  Insert %2 (Volume Serial Number: %3) into drive %1.
//
export const ERROR_WRONG_DISK = 34;

//
// MessageId: ERROR_SHARING_BUFFER_EXCEEDED
//
// MessageText:
//
//  Too many files opened for sharing.
//
export const ERROR_SHARING_BUFFER_EXCEEDED = 36;

//
// MessageId: ERROR_HANDLE_EOF
//
// MessageText:
//
//  Reached the end of the file.
//
export const ERROR_HANDLE_EOF = 38;

//
// MessageId: ERROR_HANDLE_DISK_FULL
//
// MessageText:
//
//  The disk is full.
//
export const ERROR_HANDLE_DISK_FULL = 39;

//
// MessageId: ERROR_NOT_SUPPORTED
//
// MessageText:
//
//  The request is not supported.
//
export const ERROR_NOT_SUPPORTED = 50;

//
// MessageId: ERROR_REM_NOT_LIST
//
// MessageText:
//
//  Windows cannot find the network path. Verify that the network path is correct and the destination computer is not busy or turned off. If Windows still cannot find the network path, contact your network administrator.
//
export const ERROR_REM_NOT_LIST = 51;

//
// MessageId: ERROR_DUP_NAME
//
// MessageText:
//
//  You were not connected because a duplicate name exists on the network. Go to System in Control Panel to change the computer name and try again.
//
export const ERROR_DUP_NAME = 52;

//
// MessageId: ERROR_BAD_NETPATH
//
// MessageText:
//
//  The network path was not found.
//
export const ERROR_BAD_NETPATH = 53;

//
// MessageId: ERROR_NETWORK_BUSY
//
// MessageText:
//
//  The network is busy.
//
export const ERROR_NETWORK_BUSY = 54;

//
// MessageId: ERROR_DEV_NOT_EXIST
//
// MessageText:
//
//  The specified network resource or device is no longer available.
//
export const ERROR_DEV_NOT_EXIST = 55; // dderror

//
// MessageId: ERROR_TOO_MANY_CMDS
//
// MessageText:
//
//  The network BIOS command limit has been reached.
//
export const ERROR_TOO_MANY_CMDS = 56;

//
// MessageId: ERROR_ADAP_HDW_ERR
//
// MessageText:
//
//  A network adapter hardware error occurred.
//
export const ERROR_ADAP_HDW_ERR = 57;

//
// MessageId: ERROR_BAD_NET_RESP
//
// MessageText:
//
//  The specified server cannot perform the requested operation.
//
export const ERROR_BAD_NET_RESP = 58;

//
// MessageId: ERROR_UNEXP_NET_ERR
//
// MessageText:
//
//  An unexpected network error occurred.
//
export const ERROR_UNEXP_NET_ERR = 59;

//
// MessageId: ERROR_BAD_REM_ADAP
//
// MessageText:
//
//  The remote adapter is not compatible.
//
export const ERROR_BAD_REM_ADAP = 60;

//
// MessageId: ERROR_PRINTQ_FULL
//
// MessageText:
//
//  The printer queue is full.
//
export const ERROR_PRINTQ_FULL = 61;

//
// MessageId: ERROR_NO_SPOOL_SPACE
//
// MessageText:
//
//  Space to store the file waiting to be printed is not available on the server.
//
export const ERROR_NO_SPOOL_SPACE = 62;

//
// MessageId: ERROR_PRINT_CANCELLED
//
// MessageText:
//
//  Your file waiting to be printed was deleted.
//
export const ERROR_PRINT_CANCELLED = 63;

//
// MessageId: ERROR_NETNAME_DELETED
//
// MessageText:
//
//  The specified network name is no longer available.
//
export const ERROR_NETNAME_DELETED = 64;

//
// MessageId: ERROR_NETWORK_ACCESS_DENIED
//
// MessageText:
//
//  Network access is denied.
//
export const ERROR_NETWORK_ACCESS_DENIED = 65;

//
// MessageId: ERROR_BAD_DEV_TYPE
//
// MessageText:
//
//  The network resource type is not correct.
//
export const ERROR_BAD_DEV_TYPE = 66;

//
// MessageId: ERROR_BAD_NET_NAME
//
// MessageText:
//
//  The network name cannot be found.
//
export const ERROR_BAD_NET_NAME = 67;

//
// MessageId: ERROR_TOO_MANY_NAMES
//
// MessageText:
//
//  The name limit for the local computer network adapter card was exceeded.
//
export const ERROR_TOO_MANY_NAMES = 68;

//
// MessageId: ERROR_TOO_MANY_SESS
//
// MessageText:
//
//  The network BIOS session limit was exceeded.
//
export const ERROR_TOO_MANY_SESS = 69;

//
// MessageId: ERROR_SHARING_PAUSED
//
// MessageText:
//
//  The remote server has been paused or is in the process of being started.
//
export const ERROR_SHARING_PAUSED = 70;

//
// MessageId: ERROR_REQ_NOT_ACCEP
//
// MessageText:
//
//  No more connections can be made to this remote computer at this time because there are already as many connections as the computer can accept.
//
export const ERROR_REQ_NOT_ACCEP = 71;

//
// MessageId: ERROR_REDIR_PAUSED
//
// MessageText:
//
//  The specified printer or disk device has been paused.
//
export const ERROR_REDIR_PAUSED = 72;

//
// MessageId: ERROR_FILE_EXISTS
//
// MessageText:
//
//  The file exists.
//
export const ERROR_FILE_EXISTS = 80;

//
// MessageId: ERROR_CANNOT_MAKE
//
// MessageText:
//
//  The directory or file cannot be created.
//
export const ERROR_CANNOT_MAKE = 82;

//
// MessageId: ERROR_FAIL_I24
//
// MessageText:
//
//  Fail on INT 24.
//
export const ERROR_FAIL_I24 = 83;

//
// MessageId: ERROR_OUT_OF_STRUCTURES
//
// MessageText:
//
//  Storage to process this request is not available.
//
export const ERROR_OUT_OF_STRUCTURES = 84;

//
// MessageId: ERROR_ALREADY_ASSIGNED
//
// MessageText:
//
//  The local device name is already in use.
//
export const ERROR_ALREADY_ASSIGNED = 85;

//
// MessageId: ERROR_INVALID_PASSWORD
//
// MessageText:
//
//  The specified network password is not correct.
//
export const ERROR_INVALID_PASSWORD = 86;

//
// MessageId: ERROR_INVALID_PARAMETER
//
// MessageText:
//
//  The parameter is incorrect.
//
export const ERROR_INVALID_PARAMETER = 87; // dderror

//
// MessageId: ERROR_NET_WRITE_FAULT
//
// MessageText:
//
//  A write fault occurred on the network.
//
export const ERROR_NET_WRITE_FAULT = 88;

//
// MessageId: ERROR_NO_PROC_SLOTS
//
// MessageText:
//
//  The system cannot start another process at this time.
//
export const ERROR_NO_PROC_SLOTS = 89;

//
// MessageId: ERROR_TOO_MANY_SEMAPHORES
//
// MessageText:
//
//  Cannot create another system semaphore.
//
export const ERROR_TOO_MANY_SEMAPHORES = 100;

//
// MessageId: ERROR_EXCL_SEM_ALREADY_OWNED
//
// MessageText:
//
//  The exclusive semaphore is owned by another process.
//
export const ERROR_EXCL_SEM_ALREADY_OWNED = 101;

//
// MessageId: ERROR_SEM_IS_SET
//
// MessageText:
//
//  The semaphore is set and cannot be closed.
//
export const ERROR_SEM_IS_SET = 102;

//
// MessageId: ERROR_TOO_MANY_SEM_REQUESTS
//
// MessageText:
//
//  The semaphore cannot be set again.
//
export const ERROR_TOO_MANY_SEM_REQUESTS = 103;

//
// MessageId: ERROR_INVALID_AT_INTERRUPT_TIME
//
// MessageText:
//
//  Cannot request exclusive semaphores at interrupt time.
//
export const ERROR_INVALID_AT_INTERRUPT_TIME = 104;

//
// MessageId: ERROR_SEM_OWNER_DIED
//
// MessageText:
//
//  The previous ownership of this semaphore has ended.
//
export const ERROR_SEM_OWNER_DIED = 105;

//
// MessageId: ERROR_SEM_USER_LIMIT
//
// MessageText:
//
//  Insert the diskette for drive %1.
//
export const ERROR_SEM_USER_LIMIT = 106;

//
// MessageId: ERROR_DISK_CHANGE
//
// MessageText:
//
//  The program stopped because an alternate diskette was not inserted.
//
export const ERROR_DISK_CHANGE = 107;

//
// MessageId: ERROR_DRIVE_LOCKED
//
// MessageText:
//
//  The disk is in use or locked by another process.
//
export const ERROR_DRIVE_LOCKED = 108;

//
// MessageId: ERROR_BROKEN_PIPE
//
// MessageText:
//
//  The pipe has been ended.
//
export const ERROR_BROKEN_PIPE = 109;

//
// MessageId: ERROR_OPEN_FAILED
//
// MessageText:
//
//  The system cannot open the device or file specified.
//
export const ERROR_OPEN_FAILED = 110;

//
// MessageId: ERROR_BUFFER_OVERFLOW
//
// MessageText:
//
//  The file name is too long.
//
export const ERROR_BUFFER_OVERFLOW = 111;

//
// MessageId: ERROR_DISK_FULL
//
// MessageText:
//
//  There is not enough space on the disk.
//
export const ERROR_DISK_FULL = 112;

//
// MessageId: ERROR_NO_MORE_SEARCH_HANDLES
//
// MessageText:
//
//  No more internal file identifiers available.
//
export const ERROR_NO_MORE_SEARCH_HANDLES = 113;

//
// MessageId: ERROR_INVALID_TARGET_HANDLE
//
// MessageText:
//
//  The target internal file identifier is incorrect.
//
export const ERROR_INVALID_TARGET_HANDLE = 114;

//
// MessageId: ERROR_INVALID_CATEGORY
//
// MessageText:
//
//  The IOCTL call made by the application program is not correct.
//
export const ERROR_INVALID_CATEGORY = 117;

//
// MessageId: ERROR_INVALID_VERIFY_SWITCH
//
// MessageText:
//
//  The verify-on-write switch parameter value is not correct.
//
export const ERROR_INVALID_VERIFY_SWITCH = 118;

//
// MessageId: ERROR_BAD_DRIVER_LEVEL
//
// MessageText:
//
//  The system does not support the command requested.
//
export const ERROR_BAD_DRIVER_LEVEL = 119;

//
// MessageId: ERROR_CALL_NOT_IMPLEMENTED
//
// MessageText:
//
//  This function is not supported on this system.
//
export const ERROR_CALL_NOT_IMPLEMENTED = 120;

//
// MessageId: ERROR_SEM_TIMEOUT
//
// MessageText:
//
//  The semaphore timeout period has expired.
//
export const ERROR_SEM_TIMEOUT = 121;

//
// MessageId: ERROR_INSUFFICIENT_BUFFER
//
// MessageText:
//
//  The data area passed to a system call is too small.
//
export const ERROR_INSUFFICIENT_BUFFER = 122; // dderror

//
// MessageId: ERROR_INVALID_NAME
//
// MessageText:
//
//  The filename, directory name, or volume label syntax is incorrect.
//
export const ERROR_INVALID_NAME = 123; // dderror

//
// MessageId: ERROR_INVALID_LEVEL
//
// MessageText:
//
//  The system call level is not correct.
//
export const ERROR_INVALID_LEVEL = 124;

//
// MessageId: ERROR_NO_VOLUME_LABEL
//
// MessageText:
//
//  The disk has no volume label.
//
export const ERROR_NO_VOLUME_LABEL = 125;

//
// MessageId: ERROR_MOD_NOT_FOUND
//
// MessageText:
//
//  The specified module could not be found.
//
export const ERROR_MOD_NOT_FOUND = 126;

//
// MessageId: ERROR_PROC_NOT_FOUND
//
// MessageText:
//
//  The specified procedure could not be found.
//
export const ERROR_PROC_NOT_FOUND = 127;

//
// MessageId: ERROR_WAIT_NO_CHILDREN
//
// MessageText:
//
//  There are no child processes to wait for.
//
export const ERROR_WAIT_NO_CHILDREN = 128;

//
// MessageId: ERROR_CHILD_NOT_COMPLETE
//
// MessageText:
//
//  The %1 application cannot be run in Win32 mode.
//
export const ERROR_CHILD_NOT_COMPLETE = 129;

//
// MessageId: ERROR_DIRECT_ACCESS_HANDLE
//
// MessageText:
//
//  Attempt to use a file handle to an open disk partition for an operation other than raw disk I/O.
//
export const ERROR_DIRECT_ACCESS_HANDLE = 130;

//
// MessageId: ERROR_NEGATIVE_SEEK
//
// MessageText:
//
//  An attempt was made to move the file pointer before the beginning of the file.
//
export const ERROR_NEGATIVE_SEEK = 131;

//
// MessageId: ERROR_SEEK_ON_DEVICE
//
// MessageText:
//
//  The file pointer cannot be set on the specified device or file.
//
export const ERROR_SEEK_ON_DEVICE = 132;

//
// MessageId: ERROR_IS_JOIN_TARGET
//
// MessageText:
//
//  A JOIN or SUBST command cannot be used for a drive that contains previously joined drives.
//
export const ERROR_IS_JOIN_TARGET = 133;

//
// MessageId: ERROR_IS_JOINED
//
// MessageText:
//
//  An attempt was made to use a JOIN or SUBST command on a drive that has already been joined.
//
export const ERROR_IS_JOINED = 134;

//
// MessageId: ERROR_IS_SUBSTED
//
// MessageText:
//
//  An attempt was made to use a JOIN or SUBST command on a drive that has already been substituted.
//
export const ERROR_IS_SUBSTED = 135;

//
// MessageId: ERROR_NOT_JOINED
//
// MessageText:
//
//  The system tried to delete the JOIN of a drive that is not joined.
//
export const ERROR_NOT_JOINED = 136;

//
// MessageId: ERROR_NOT_SUBSTED
//
// MessageText:
//
//  The system tried to delete the substitution of a drive that is not substituted.
//
export const ERROR_NOT_SUBSTED = 137;

//
// MessageId: ERROR_JOIN_TO_JOIN
//
// MessageText:
//
//  The system tried to join a drive to a directory on a joined drive.
//
export const ERROR_JOIN_TO_JOIN = 138;

//
// MessageId: ERROR_SUBST_TO_SUBST
//
// MessageText:
//
//  The system tried to substitute a drive to a directory on a substituted drive.
//
export const ERROR_SUBST_TO_SUBST = 139;

//
// MessageId: ERROR_JOIN_TO_SUBST
//
// MessageText:
//
//  The system tried to join a drive to a directory on a substituted drive.
//
export const ERROR_JOIN_TO_SUBST = 140;

//
// MessageId: ERROR_SUBST_TO_JOIN
//
// MessageText:
//
//  The system tried to SUBST a drive to a directory on a joined drive.
//
export const ERROR_SUBST_TO_JOIN = 141;

//
// MessageId: ERROR_BUSY_DRIVE
//
// MessageText:
//
//  The system cannot perform a JOIN or SUBST at this time.
//
export const ERROR_BUSY_DRIVE = 142;

//
// MessageId: ERROR_SAME_DRIVE
//
// MessageText:
//
//  The system cannot join or substitute a drive to or for a directory on the same drive.
//
export const ERROR_SAME_DRIVE = 143;

//
// MessageId: ERROR_DIR_NOT_ROOT
//
// MessageText:
//
//  The directory is not a subdirectory of the root directory.
//
export const ERROR_DIR_NOT_ROOT = 144;

//
// MessageId: ERROR_DIR_NOT_EMPTY
//
// MessageText:
//
//  The directory is not empty.
//
export const ERROR_DIR_NOT_EMPTY = 145;

//
// MessageId: ERROR_IS_SUBST_PATH
//
// MessageText:
//
//  The path specified is being used in a substitute.
//
export const ERROR_IS_SUBST_PATH = 146;

//
// MessageId: ERROR_IS_JOIN_PATH
//
// MessageText:
//
//  Not enough resources are available to process this command.
//
export const ERROR_IS_JOIN_PATH = 147;

//
// MessageId: ERROR_PATH_BUSY
//
// MessageText:
//
//  The path specified cannot be used at this time.
//
export const ERROR_PATH_BUSY = 148;

//
// MessageId: ERROR_IS_SUBST_TARGET
//
// MessageText:
//
//  An attempt was made to join or substitute a drive for which a directory on the drive is the target of a previous substitute.
//
export const ERROR_IS_SUBST_TARGET = 149;

//
// MessageId: ERROR_SYSTEM_TRACE
//
// MessageText:
//
//  System trace information was not specified in your CONFIG.SYS file, or tracing is disallowed.
//
export const ERROR_SYSTEM_TRACE = 150;

//
// MessageId: ERROR_INVALID_EVENT_COUNT
//
// MessageText:
//
//  The number of specified semaphore events for DosMuxSemWait is not correct.
//
export const ERROR_INVALID_EVENT_COUNT = 151;

//
// MessageId: ERROR_TOO_MANY_MUXWAITERS
//
// MessageText:
//
//  DosMuxSemWait did not execute; too many semaphores are already set.
//
export const ERROR_TOO_MANY_MUXWAITERS = 152;

//
// MessageId: ERROR_INVALID_LIST_FORMAT
//
// MessageText:
//
//  The DosMuxSemWait list is not correct.
//
export const ERROR_INVALID_LIST_FORMAT = 153;

//
// MessageId: ERROR_LABEL_TOO_LONG
//
// MessageText:
//
//  The volume label you entered exceeds the label character limit of the target file system.
//
export const ERROR_LABEL_TOO_LONG = 154;

//
// MessageId: ERROR_TOO_MANY_TCBS
//
// MessageText:
//
//  Cannot create another thread.
//
export const ERROR_TOO_MANY_TCBS = 155;

//
// MessageId: ERROR_SIGNAL_REFUSED
//
// MessageText:
//
//  The recipient process has refused the signal.
//
export const ERROR_SIGNAL_REFUSED = 156;

//
// MessageId: ERROR_DISCARDED
//
// MessageText:
//
//  The segment is already discarded and cannot be locked.
//
export const ERROR_DISCARDED = 157;

//
// MessageId: ERROR_NOT_LOCKED
//
// MessageText:
//
//  The segment is already unlocked.
//
export const ERROR_NOT_LOCKED = 158;

//
// MessageId: ERROR_BAD_THREADID_ADDR
//
// MessageText:
//
//  The address for the thread ID is not correct.
//
export const ERROR_BAD_THREADID_ADDR = 159;

//
// MessageId: ERROR_BAD_ARGUMENTS
//
// MessageText:
//
//  One or more arguments are not correct.
//
export const ERROR_BAD_ARGUMENTS = 160;

//
// MessageId: ERROR_BAD_PATHNAME
//
// MessageText:
//
//  The specified path is invalid.
//
export const ERROR_BAD_PATHNAME = 161;

//
// MessageId: ERROR_SIGNAL_PENDING
//
// MessageText:
//
//  A signal is already pending.
//
export const ERROR_SIGNAL_PENDING = 162;

//
// MessageId: ERROR_MAX_THRDS_REACHED
//
// MessageText:
//
//  No more threads can be created in the system.
//
export const ERROR_MAX_THRDS_REACHED = 164;

//
// MessageId: ERROR_LOCK_FAILED
//
// MessageText:
//
//  Unable to lock a region of a file.
//
export const ERROR_LOCK_FAILED = 167;

//
// MessageId: ERROR_BUSY
//
// MessageText:
//
//  The requested resource is in use.
//
export const ERROR_BUSY = 170; // dderror

//
// MessageId: ERROR_CANCEL_VIOLATION
//
// MessageText:
//
//  A lock request was not outstanding for the supplied cancel region.
//
export const ERROR_CANCEL_VIOLATION = 173;

//
// MessageId: ERROR_ATOMIC_LOCKS_NOT_SUPPORTED
//
// MessageText:
//
//  The file system does not support atomic changes to the lock type.
//
export const ERROR_ATOMIC_LOCKS_NOT_SUPPORTED = 174;

//
// MessageId: ERROR_INVALID_SEGMENT_NUMBER
//
// MessageText:
//
//  The system detected a segment number that was not correct.
//
export const ERROR_INVALID_SEGMENT_NUMBER = 180;

//
// MessageId: ERROR_INVALID_ORDINAL
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INVALID_ORDINAL = 182;

//
// MessageId: ERROR_ALREADY_EXISTS
//
// MessageText:
//
//  Cannot create a file when that file already exists.
//
export const ERROR_ALREADY_EXISTS = 183;

//
// MessageId: ERROR_INVALID_FLAG_NUMBER
//
// MessageText:
//
//  The flag passed is not correct.
//
export const ERROR_INVALID_FLAG_NUMBER = 186;

//
// MessageId: ERROR_SEM_NOT_FOUND
//
// MessageText:
//
//  The specified system semaphore name was not found.
//
export const ERROR_SEM_NOT_FOUND = 187;

//
// MessageId: ERROR_INVALID_STARTING_CODESEG
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INVALID_STARTING_CODESEG = 188;

//
// MessageId: ERROR_INVALID_STACKSEG
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INVALID_STACKSEG = 189;

//
// MessageId: ERROR_INVALID_MODULETYPE
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INVALID_MODULETYPE = 190;

//
// MessageId: ERROR_INVALID_EXE_SIGNATURE
//
// MessageText:
//
//  Cannot run %1 in Win32 mode.
//
export const ERROR_INVALID_EXE_SIGNATURE = 191;

//
// MessageId: ERROR_EXE_MARKED_INVALID
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_EXE_MARKED_INVALID = 192;

//
// MessageId: ERROR_BAD_EXE_FORMAT
//
// MessageText:
//
//  %1 is not a valid Win32 application.
//
export const ERROR_BAD_EXE_FORMAT = 193;

//
// MessageId: ERROR_ITERATED_DATA_EXCEEDS_64k
//
// MessageText:
//
//  The operating system cannot run %1.
//
// deno-lint-ignore camelcase
export const ERROR_ITERATED_DATA_EXCEEDS_64k = 194;

//
// MessageId: ERROR_INVALID_MINALLOCSIZE
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INVALID_MINALLOCSIZE = 195;

//
// MessageId: ERROR_DYNLINK_FROM_INVALID_RING
//
// MessageText:
//
//  The operating system cannot run this application program.
//
export const ERROR_DYNLINK_FROM_INVALID_RING = 196;

//
// MessageId: ERROR_IOPL_NOT_ENABLED
//
// MessageText:
//
//  The operating system is not presently configured to run this application.
//
export const ERROR_IOPL_NOT_ENABLED = 197;

//
// MessageId: ERROR_INVALID_SEGDPL
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INVALID_SEGDPL = 198;

//
// MessageId: ERROR_AUTODATASEG_EXCEEDS_64k
//
// MessageText:
//
//  The operating system cannot run this application program.
//
// deno-lint-ignore camelcase
export const ERROR_AUTODATASEG_EXCEEDS_64k = 199;

//
// MessageId: ERROR_RING2SEG_MUST_BE_MOVABLE
//
// MessageText:
//
//  The code segment cannot be greater than or equal to 64K.
//
export const ERROR_RING2SEG_MUST_BE_MOVABLE = 200;

//
// MessageId: ERROR_RELOC_CHAIN_XEEDS_SEGLIM
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_RELOC_CHAIN_XEEDS_SEGLIM = 201;

//
// MessageId: ERROR_INFLOOP_IN_RELOC_CHAIN
//
// MessageText:
//
//  The operating system cannot run %1.
//
export const ERROR_INFLOOP_IN_RELOC_CHAIN = 202;

//
// MessageId: ERROR_ENVVAR_NOT_FOUND
//
// MessageText:
//
//  The system could not find the environment option that was entered.
//
export const ERROR_ENVVAR_NOT_FOUND = 203;

//
// MessageId: ERROR_NO_SIGNAL_SENT
//
// MessageText:
//
//  No process in the command subtree has a signal handler.
//
export const ERROR_NO_SIGNAL_SENT = 205;

//
// MessageId: ERROR_FILENAME_EXCED_RANGE
//
// MessageText:
//
//  The filename or extension is too long.
//
export const ERROR_FILENAME_EXCED_RANGE = 206;

//
// MessageId: ERROR_RING2_STACK_IN_USE
//
// MessageText:
//
//  The ring 2 stack is in use.
//
export const ERROR_RING2_STACK_IN_USE = 207;

//
// MessageId: ERROR_META_EXPANSION_TOO_LONG
//
// MessageText:
//
//  The global filename characters, * or ?, are entered incorrectly or too many global filename characters are specified.
//
export const ERROR_META_EXPANSION_TOO_LONG = 208;

//
// MessageId: ERROR_INVALID_SIGNAL_NUMBER
//
// MessageText:
//
//  The signal being posted is not correct.
//
export const ERROR_INVALID_SIGNAL_NUMBER = 209;

//
// MessageId: ERROR_THREAD_1_INACTIVE
//
// MessageText:
//
//  The signal handler cannot be set.
//
export const ERROR_THREAD_1_INACTIVE = 210;

//
// MessageId: ERROR_LOCKED
//
// MessageText:
//
//  The segment is locked and cannot be reallocated.
//
export const ERROR_LOCKED = 212;

//
// MessageId: ERROR_TOO_MANY_MODULES
//
// MessageText:
//
//  Too many dynamic-link modules are attached to this program or dynamic-link module.
//
export const ERROR_TOO_MANY_MODULES = 214;

//
// MessageId: ERROR_NESTING_NOT_ALLOWED
//
// MessageText:
//
//  Cannot nest calls to LoadModule.
//
export const ERROR_NESTING_NOT_ALLOWED = 215;

//
// MessageId: ERROR_EXE_MACHINE_TYPE_MISMATCH
//
// MessageText:
//
//  The image file %1 is valid, but is for a machine type other than the current machine.
//
export const ERROR_EXE_MACHINE_TYPE_MISMATCH = 216;

//
// MessageId: ERROR_EXE_CANNOT_MODIFY_SIGNED_BINARY
//
// MessageText:
//
//  The image file %1 is signed, unable to modify.
//
export const ERROR_EXE_CANNOT_MODIFY_SIGNED_BINARY = 217;

//
// MessageId: ERROR_EXE_CANNOT_MODIFY_STRONG_SIGNED_BINARY
//
// MessageText:
//
//  The image file %1 is strong signed, unable to modify.
//
export const ERROR_EXE_CANNOT_MODIFY_STRONG_SIGNED_BINARY = 218;

//
// MessageId: ERROR_BAD_PIPE
//
// MessageText:
//
//  The pipe state is invalid.
//
export const ERROR_BAD_PIPE = 230;

//
// MessageId: ERROR_PIPE_BUSY
//
// MessageText:
//
//  All pipe instances are busy.
//
export const ERROR_PIPE_BUSY = 231;

//
// MessageId: ERROR_NO_DATA
//
// MessageText:
//
//  The pipe is being closed.
//
export const ERROR_NO_DATA = 232;

//
// MessageId: ERROR_PIPE_NOT_CONNECTED
//
// MessageText:
//
//  No process is on the other end of the pipe.
//
export const ERROR_PIPE_NOT_CONNECTED = 233;

//
// MessageId: ERROR_MORE_DATA
//
// MessageText:
//
//  More data is available.
//
export const ERROR_MORE_DATA = 234; // dderror

//
// MessageId: ERROR_VC_DISCONNECTED
//
// MessageText:
//
//  The session was canceled.
//
export const ERROR_VC_DISCONNECTED = 240;

//
// MessageId: ERROR_INVALID_EA_NAME
//
// MessageText:
//
//  The specified extended attribute name was invalid.
//
export const ERROR_INVALID_EA_NAME = 254;

//
// MessageId: ERROR_EA_LIST_INCONSISTENT
//
// MessageText:
//
//  The extended attributes are inconsistent.
//
export const ERROR_EA_LIST_INCONSISTENT = 255;

//
// MessageId: WAIT_TIMEOUT
//
// MessageText:
//
//  The wait operation timed out.
//
export const WAIT_TIMEOUT = 258; // dderror

//
// MessageId: ERROR_NO_MORE_ITEMS
//
// MessageText:
//
//  No more data is available.
//
export const ERROR_NO_MORE_ITEMS = 259;

//
// MessageId: ERROR_CANNOT_COPY
//
// MessageText:
//
//  The copy functions cannot be used.
//
export const ERROR_CANNOT_COPY = 266;

//
// MessageId: ERROR_DIRECTORY
//
// MessageText:
//
//  The directory name is invalid.
//
export const ERROR_DIRECTORY = 267;

//
// MessageId: ERROR_EAS_DIDNT_FIT
//
// MessageText:
//
//  The extended attributes did not fit in the buffer.
//
export const ERROR_EAS_DIDNT_FIT = 275;

//
// MessageId: ERROR_EA_FILE_CORRUPT
//
// MessageText:
//
//  The extended attribute file on the mounted file system is corrupt.
//
export const ERROR_EA_FILE_CORRUPT = 276;

//
// MessageId: ERROR_EA_TABLE_FULL
//
// MessageText:
//
//  The extended attribute table file is full.
//
export const ERROR_EA_TABLE_FULL = 277;

//
// MessageId: ERROR_INVALID_EA_HANDLE
//
// MessageText:
//
//  The specified extended attribute handle is invalid.
//
export const ERROR_INVALID_EA_HANDLE = 278;

//
// MessageId: ERROR_EAS_NOT_SUPPORTED
//
// MessageText:
//
//  The mounted file system does not support extended attributes.
//
export const ERROR_EAS_NOT_SUPPORTED = 282;

//
// MessageId: ERROR_NOT_OWNER
//
// MessageText:
//
//  Attempt to release mutex not owned by caller.
//
export const ERROR_NOT_OWNER = 288;

//
// MessageId: ERROR_TOO_MANY_POSTS
//
// MessageText:
//
//  Too many posts were made to a semaphore.
//
export const ERROR_TOO_MANY_POSTS = 298;

//
// MessageId: ERROR_PARTIAL_COPY
//
// MessageText:
//
//  Only part of a ReadProcessMemory or WriteProcessMemory request was completed.
//
export const ERROR_PARTIAL_COPY = 299;

//
// MessageId: ERROR_OPLOCK_NOT_GRANTED
//
// MessageText:
//
//  The oplock request is denied.
//
export const ERROR_OPLOCK_NOT_GRANTED = 300;

//
// MessageId: ERROR_INVALID_OPLOCK_PROTOCOL
//
// MessageText:
//
//  An invalid oplock acknowledgment was received by the system.
//
export const ERROR_INVALID_OPLOCK_PROTOCOL = 301;

//
// MessageId: ERROR_DISK_TOO_FRAGMENTED
//
// MessageText:
//
//  The volume is too fragmented to complete this operation.
//
export const ERROR_DISK_TOO_FRAGMENTED = 302;

//
// MessageId: ERROR_DELETE_PENDING
//
// MessageText:
//
//  The file cannot be opened because it is in the process of being deleted.
//
export const ERROR_DELETE_PENDING = 303;

//
// MessageId: ERROR_MR_MID_NOT_FOUND
//
// MessageText:
//
//  The system cannot find message text for message number 0x%1 in the message file for %2.
//
export const ERROR_MR_MID_NOT_FOUND = 317;

//
// MessageId: ERROR_SCOPE_NOT_FOUND
//
// MessageText:
//
//  The scope specified was not found.
//
export const ERROR_SCOPE_NOT_FOUND = 318;

//
// MessageId: ERROR_INVALID_ADDRESS
//
// MessageText:
//
//  Attempt to access invalid address.
//
export const ERROR_INVALID_ADDRESS = 487;

//
// MessageId: ERROR_ARITHMETIC_OVERFLOW
//
// MessageText:
//
//  Arithmetic result exceeded 32 bits.
//
export const ERROR_ARITHMETIC_OVERFLOW = 534;

//
// MessageId: ERROR_PIPE_CONNECTED
//
// MessageText:
//
//  There is a process on other end of the pipe.
//
export const ERROR_PIPE_CONNECTED = 535;

//
// MessageId: ERROR_PIPE_LISTENING
//
// MessageText:
//
//  Waiting for a process to open the other end of the pipe.
//
export const ERROR_PIPE_LISTENING = 536;

//
// MessageId: ERROR_EA_ACCESS_DENIED
//
// MessageText:
//
//  Access to the extended attribute was denied.
//
export const ERROR_EA_ACCESS_DENIED = 994;

//
// MessageId: ERROR_OPERATION_ABORTED
//
// MessageText:
//
//  The I/O operation has been aborted because of either a thread exit or an application request.
//
export const ERROR_OPERATION_ABORTED = 995;

//
// MessageId: ERROR_IO_INCOMPLETE
//
// MessageText:
//
//  Overlapped I/O event is not in a signaled state.
//
export const ERROR_IO_INCOMPLETE = 996;

//
// MessageId: ERROR_IO_PENDING
//
// MessageText:
//
//  Overlapped I/O operation is in progress.
//
export const ERROR_IO_PENDING = 997; // dderror

//
// MessageId: ERROR_NOACCESS
//
// MessageText:
//
//  Invalid access to memory location.
//
export const ERROR_NOACCESS = 998;

//
// MessageId: ERROR_SWAPERROR
//
// MessageText:
//
//  Error performing inpage operation.
//
export const ERROR_SWAPERROR = 999;

//
// MessageId: ERROR_STACK_OVERFLOW
//
// MessageText:
//
//  Recursion too deep; the stack overflowed.
//
export const ERROR_STACK_OVERFLOW = 1001;

//
// MessageId: ERROR_INVALID_MESSAGE
//
// MessageText:
//
//  The window cannot act on the sent message.
//
export const ERROR_INVALID_MESSAGE = 1002;

//
// MessageId: ERROR_CAN_NOT_COMPLETE
//
// MessageText:
//
//  Cannot complete this function.
//
export const ERROR_CAN_NOT_COMPLETE = 1003;

//
// MessageId: ERROR_INVALID_FLAGS
//
// MessageText:
//
//  Invalid flags.
//
export const ERROR_INVALID_FLAGS = 1004;

//
// MessageId: ERROR_UNRECOGNIZED_VOLUME
//
// MessageText:
//
//  The volume does not contain a recognized file system.
//  Please make sure that all required file system drivers are loaded and that the volume is not corrupted.
//
export const ERROR_UNRECOGNIZED_VOLUME = 1005;

//
// MessageId: ERROR_FILE_INVALID
//
// MessageText:
//
//  The volume for a file has been externally altered so that the opened file is no longer valid.
//
export const ERROR_FILE_INVALID = 1006;

//
// MessageId: ERROR_FULLSCREEN_MODE
//
// MessageText:
//
//  The requested operation cannot be performed in full-screen mode.
//
export const ERROR_FULLSCREEN_MODE = 1007;

//
// MessageId: ERROR_NO_TOKEN
//
// MessageText:
//
//  An attempt was made to reference a token that does not exist.
//
export const ERROR_NO_TOKEN = 1008;

//
// MessageId: ERROR_BADDB
//
// MessageText:
//
//  The configuration registry database is corrupt.
//
export const ERROR_BADDB = 1009;

//
// MessageId: ERROR_BADKEY
//
// MessageText:
//
//  The configuration registry key is invalid.
//
export const ERROR_BADKEY = 1010;

//
// MessageId: ERROR_CANTOPEN
//
// MessageText:
//
//  The configuration registry key could not be opened.
//
export const ERROR_CANTOPEN = 1011;

//
// MessageId: ERROR_CANTREAD
//
// MessageText:
//
//  The configuration registry key could not be read.
//
export const ERROR_CANTREAD = 1012;

//
// MessageId: ERROR_CANTWRITE
//
// MessageText:
//
//  The configuration registry key could not be written.
//
export const ERROR_CANTWRITE = 1013;

//
// MessageId: ERROR_REGISTRY_RECOVERED
//
// MessageText:
//
//  One of the files in the registry database had to be recovered by use of a log or alternate copy. The recovery was successful.
//
export const ERROR_REGISTRY_RECOVERED = 1014;

//
// MessageId: ERROR_REGISTRY_CORRUPT
//
// MessageText:
//
//  The registry is corrupted. The structure of one of the files containing registry data is corrupted, or the system's memory image of the file is corrupted, or the file could not be recovered because the alternate copy or log was absent or corrupted.
//
export const ERROR_REGISTRY_CORRUPT = 1015;

//
// MessageId: ERROR_REGISTRY_IO_FAILED
//
// MessageText:
//
//  An I/O operation initiated by the registry failed unrecoverably. The registry could not read in, or write out, or flush, one of the files that contain the system's image of the registry.
//
export const ERROR_REGISTRY_IO_FAILED = 1016;

//
// MessageId: ERROR_NOT_REGISTRY_FILE
//
// MessageText:
//
//  The system has attempted to load or restore a file into the registry, but the specified file is not in a registry file format.
//
export const ERROR_NOT_REGISTRY_FILE = 1017;

//
// MessageId: ERROR_KEY_DELETED
//
// MessageText:
//
//  Illegal operation attempted on a registry key that has been marked for deletion.
//
export const ERROR_KEY_DELETED = 1018;

//
// MessageId: ERROR_NO_LOG_SPACE
//
// MessageText:
//
//  System could not allocate the required space in a registry log.
//
export const ERROR_NO_LOG_SPACE = 1019;

//
// MessageId: ERROR_KEY_HAS_CHILDREN
//
// MessageText:
//
//  Cannot create a symbolic link in a registry key that already has subkeys or values.
//
export const ERROR_KEY_HAS_CHILDREN = 1020;

//
// MessageId: ERROR_CHILD_MUST_BE_VOLATILE
//
// MessageText:
//
//  Cannot create a stable subkey under a volatile parent key.
//
export const ERROR_CHILD_MUST_BE_VOLATILE = 1021;

//
// MessageId: ERROR_NOTIFY_ENUM_DIR
//
// MessageText:
//
//  A notify change request is being completed and the information is not being returned in the caller's buffer. The caller now needs to enumerate the files to find the changes.
//
export const ERROR_NOTIFY_ENUM_DIR = 1022;

//
// MessageId: ERROR_DEPENDENT_SERVICES_RUNNING
//
// MessageText:
//
//  A stop control has been sent to a service that other running services are dependent on.
//
export const ERROR_DEPENDENT_SERVICES_RUNNING = 1051;

//
// MessageId: ERROR_INVALID_SERVICE_CONTROL
//
// MessageText:
//
//  The requested control is not valid for this service.
//
export const ERROR_INVALID_SERVICE_CONTROL = 1052;

//
// MessageId: ERROR_SERVICE_REQUEST_TIMEOUT
//
// MessageText:
//
//  The service did not respond to the start or control request in a timely fashion.
//
export const ERROR_SERVICE_REQUEST_TIMEOUT = 1053;

//
// MessageId: ERROR_SERVICE_NO_THREAD
//
// MessageText:
//
//  A thread could not be created for the service.
//
export const ERROR_SERVICE_NO_THREAD = 1054;

//
// MessageId: ERROR_SERVICE_DATABASE_LOCKED
//
// MessageText:
//
//  The service database is locked.
//
export const ERROR_SERVICE_DATABASE_LOCKED = 1055;

//
// MessageId: ERROR_SERVICE_ALREADY_RUNNING
//
// MessageText:
//
//  An instance of the service is already running.
//
export const ERROR_SERVICE_ALREADY_RUNNING = 1056;

//
// MessageId: ERROR_INVALID_SERVICE_ACCOUNT
//
// MessageText:
//
//  The account name is invalid or does not exist, or the password is invalid for the account name specified.
//
export const ERROR_INVALID_SERVICE_ACCOUNT = 1057;

//
// MessageId: ERROR_SERVICE_DISABLED
//
// MessageText:
//
//  The service cannot be started, either because it is disabled or because it has no enabled devices associated with it.
//
export const ERROR_SERVICE_DISABLED = 1058;

//
// MessageId: ERROR_CIRCULAR_DEPENDENCY
//
// MessageText:
//
//  Circular service dependency was specified.
//
export const ERROR_CIRCULAR_DEPENDENCY = 1059;

//
// MessageId: ERROR_SERVICE_DOES_NOT_EXIST
//
// MessageText:
//
//  The specified service does not exist as an installed service.
//
export const ERROR_SERVICE_DOES_NOT_EXIST = 1060;

//
// MessageId: ERROR_SERVICE_CANNOT_ACCEPT_CTRL
//
// MessageText:
//
//  The service cannot accept control messages at this time.
//
export const ERROR_SERVICE_CANNOT_ACCEPT_CTRL = 1061;

//
// MessageId: ERROR_SERVICE_NOT_ACTIVE
//
// MessageText:
//
//  The service has not been started.
//
export const ERROR_SERVICE_NOT_ACTIVE = 1062;

//
// MessageId: ERROR_FAILED_SERVICE_CONTROLLER_CONNECT
//
// MessageText:
//
//  The service process could not connect to the service controller.
//
export const ERROR_FAILED_SERVICE_CONTROLLER_CONNECT = 1063;

//
// MessageId: ERROR_EXCEPTION_IN_SERVICE
//
// MessageText:
//
//  An exception occurred in the service when handling the control request.
//
export const ERROR_EXCEPTION_IN_SERVICE = 1064;

//
// MessageId: ERROR_DATABASE_DOES_NOT_EXIST
//
// MessageText:
//
//  The database specified does not exist.
//
export const ERROR_DATABASE_DOES_NOT_EXIST = 1065;

//
// MessageId: ERROR_SERVICE_SPECIFIC_ERROR
//
// MessageText:
//
//  The service has returned a service-specific error code.
//
export const ERROR_SERVICE_SPECIFIC_ERROR = 1066;

//
// MessageId: ERROR_PROCESS_ABORTED
//
// MessageText:
//
//  The process terminated unexpectedly.
//
export const ERROR_PROCESS_ABORTED = 1067;

//
// MessageId: ERROR_SERVICE_DEPENDENCY_FAIL
//
// MessageText:
//
//  The dependency service or group failed to start.
//
export const ERROR_SERVICE_DEPENDENCY_FAIL = 1068;

//
// MessageId: ERROR_SERVICE_LOGON_FAILED
//
// MessageText:
//
//  The service did not start due to a logon failure.
//
export const ERROR_SERVICE_LOGON_FAILED = 1069;

//
// MessageId: ERROR_SERVICE_START_HANG
//
// MessageText:
//
//  After starting, the service hung in a start-pending state.
//
export const ERROR_SERVICE_START_HANG = 1070;

//
// MessageId: ERROR_INVALID_SERVICE_LOCK
//
// MessageText:
//
//  The specified service database lock is invalid.
//
export const ERROR_INVALID_SERVICE_LOCK = 1071;

//
// MessageId: ERROR_SERVICE_MARKED_FOR_DELETE
//
// MessageText:
//
//  The specified service has been marked for deletion.
//
export const ERROR_SERVICE_MARKED_FOR_DELETE = 1072;

//
// MessageId: ERROR_SERVICE_EXISTS
//
// MessageText:
//
//  The specified service already exists.
//
export const ERROR_SERVICE_EXISTS = 1073;

//
// MessageId: ERROR_ALREADY_RUNNING_LKG
//
// MessageText:
//
//  The system is currently running with the last-known-good configuration.
//
export const ERROR_ALREADY_RUNNING_LKG = 1074;

//
// MessageId: ERROR_SERVICE_DEPENDENCY_DELETED
//
// MessageText:
//
//  The dependency service does not exist or has been marked for deletion.
//
export const ERROR_SERVICE_DEPENDENCY_DELETED = 1075;

//
// MessageId: ERROR_BOOT_ALREADY_ACCEPTED
//
// MessageText:
//
//  The current boot has already been accepted for use as the last-known-good control set.
//
export const ERROR_BOOT_ALREADY_ACCEPTED = 1076;

//
// MessageId: ERROR_SERVICE_NEVER_STARTED
//
// MessageText:
//
//  No attempts to start the service have been made since the last boot.
//
export const ERROR_SERVICE_NEVER_STARTED = 1077;

//
// MessageId: ERROR_DUPLICATE_SERVICE_NAME
//
// MessageText:
//
//  The name is already in use as either a service name or a service display name.
//
export const ERROR_DUPLICATE_SERVICE_NAME = 1078;

//
// MessageId: ERROR_DIFFERENT_SERVICE_ACCOUNT
//
// MessageText:
//
//  The account specified for this service is different from the account specified for other services running in the same process.
//
export const ERROR_DIFFERENT_SERVICE_ACCOUNT = 1079;

//
// MessageId: ERROR_CANNOT_DETECT_DRIVER_FAILURE
//
// MessageText:
//
//  Failure actions can only be set for Win32 services, not for drivers.
//
export const ERROR_CANNOT_DETECT_DRIVER_FAILURE = 1080;

//
// MessageId: ERROR_CANNOT_DETECT_PROCESS_ABORT
//
// MessageText:
//
//  This service runs in the same process as the service control manager.
//  Therefore, the service control manager cannot take action if this service's process terminates unexpectedly.
//
export const ERROR_CANNOT_DETECT_PROCESS_ABORT = 1081;

//
// MessageId: ERROR_NO_RECOVERY_PROGRAM
//
// MessageText:
//
//  No recovery program has been configured for this service.
//
export const ERROR_NO_RECOVERY_PROGRAM = 1082;

//
// MessageId: ERROR_SERVICE_NOT_IN_EXE
//
// MessageText:
//
//  The executable program that this service is configured to run in does not implement the service.
//
export const ERROR_SERVICE_NOT_IN_EXE = 1083;

//
// MessageId: ERROR_NOT_SAFEBOOT_SERVICE
//
// MessageText:
//
//  This service cannot be started in Safe Mode
//
export const ERROR_NOT_SAFEBOOT_SERVICE = 1084;

//
// MessageId: ERROR_END_OF_MEDIA
//
// MessageText:
//
//  The physical end of the tape has been reached.
//
export const ERROR_END_OF_MEDIA = 1100;

//
// MessageId: ERROR_FILEMARK_DETECTED
//
// MessageText:
//
//  A tape access reached a filemark.
//
export const ERROR_FILEMARK_DETECTED = 1101;

//
// MessageId: ERROR_BEGINNING_OF_MEDIA
//
// MessageText:
//
//  The beginning of the tape or a partition was encountered.
//
export const ERROR_BEGINNING_OF_MEDIA = 1102;

//
// MessageId: ERROR_SETMARK_DETECTED
//
// MessageText:
//
//  A tape access reached the end of a set of files.
//
export const ERROR_SETMARK_DETECTED = 1103;

//
// MessageId: ERROR_NO_DATA_DETECTED
//
// MessageText:
//
//  No more data is on the tape.
//
export const ERROR_NO_DATA_DETECTED = 1104;

//
// MessageId: ERROR_PARTITION_FAILURE
//
// MessageText:
//
//  Tape could not be partitioned.
//
export const ERROR_PARTITION_FAILURE = 1105;

//
// MessageId: ERROR_INVALID_BLOCK_LENGTH
//
// MessageText:
//
//  When accessing a new tape of a multivolume partition, the current block size is incorrect.
//
export const ERROR_INVALID_BLOCK_LENGTH = 1106;

//
// MessageId: ERROR_DEVICE_NOT_PARTITIONED
//
// MessageText:
//
//  Tape partition information could not be found when loading a tape.
//
export const ERROR_DEVICE_NOT_PARTITIONED = 1107;

//
// MessageId: ERROR_UNABLE_TO_LOCK_MEDIA
//
// MessageText:
//
//  Unable to lock the media eject mechanism.
//
export const ERROR_UNABLE_TO_LOCK_MEDIA = 1108;

//
// MessageId: ERROR_UNABLE_TO_UNLOAD_MEDIA
//
// MessageText:
//
//  Unable to unload the media.
//
export const ERROR_UNABLE_TO_UNLOAD_MEDIA = 1109;

//
// MessageId: ERROR_MEDIA_CHANGED
//
// MessageText:
//
//  The media in the drive may have changed.
//
export const ERROR_MEDIA_CHANGED = 1110;

//
// MessageId: ERROR_BUS_RESET
//
// MessageText:
//
//  The I/O bus was reset.
//
export const ERROR_BUS_RESET = 1111;

//
// MessageId: ERROR_NO_MEDIA_IN_DRIVE
//
// MessageText:
//
//  No media in drive.
//
export const ERROR_NO_MEDIA_IN_DRIVE = 1112;

//
// MessageId: ERROR_NO_UNICODE_TRANSLATION
//
// MessageText:
//
//  No mapping for the Unicode character exists in the target multi-byte code page.
//
export const ERROR_NO_UNICODE_TRANSLATION = 1113;

//
// MessageId: ERROR_DLL_INIT_FAILED
//
// MessageText:
//
//  A dynamic link library (DLL) initialization routine failed.
//
export const ERROR_DLL_INIT_FAILED = 1114;

//
// MessageId: ERROR_SHUTDOWN_IN_PROGRESS
//
// MessageText:
//
//  A system shutdown is in progress.
//
export const ERROR_SHUTDOWN_IN_PROGRESS = 1115;

//
// MessageId: ERROR_NO_SHUTDOWN_IN_PROGRESS
//
// MessageText:
//
//  Unable to abort the system shutdown because no shutdown was in progress.
//
export const ERROR_NO_SHUTDOWN_IN_PROGRESS = 1116;

//
// MessageId: ERROR_IO_DEVICE
//
// MessageText:
//
//  The request could not be performed because of an I/O device error.
//
export const ERROR_IO_DEVICE = 1117;

//
// MessageId: ERROR_SERIAL_NO_DEVICE
//
// MessageText:
//
//  No serial device was successfully initialized. The serial driver will unload.
//
export const ERROR_SERIAL_NO_DEVICE = 1118;

//
// MessageId: ERROR_IRQ_BUSY
//
// MessageText:
//
//  Unable to open a device that was sharing an interrupt request (IRQ) with other devices. At least one other device that uses that IRQ was already opened.
//
export const ERROR_IRQ_BUSY = 1119;

//
// MessageId: ERROR_MORE_WRITES
//
// MessageText:
//
//  A serial I/O operation was completed by another write to the serial port.
//  (The IOCTL_SERIAL_XOFF_COUNTER reached zero.)
//
export const ERROR_MORE_WRITES = 1120;

//
// MessageId: ERROR_COUNTER_TIMEOUT
//
// MessageText:
//
//  A serial I/O operation completed because the timeout period expired.
//  (The IOCTL_SERIAL_XOFF_COUNTER did not reach zero.)
//
export const ERROR_COUNTER_TIMEOUT = 1121;

//
// MessageId: ERROR_FLOPPY_ID_MARK_NOT_FOUND
//
// MessageText:
//
//  No ID address mark was found on the floppy disk.
//
export const ERROR_FLOPPY_ID_MARK_NOT_FOUND = 1122;

//
// MessageId: ERROR_FLOPPY_WRONG_CYLINDER
//
// MessageText:
//
//  Mismatch between the floppy disk sector ID field and the floppy disk controller track address.
//
export const ERROR_FLOPPY_WRONG_CYLINDER = 1123;

//
// MessageId: ERROR_FLOPPY_UNKNOWN_ERROR
//
// MessageText:
//
//  The floppy disk controller reported an error that is not recognized by the floppy disk driver.
//
export const ERROR_FLOPPY_UNKNOWN_ERROR = 1124;

//
// MessageId: ERROR_FLOPPY_BAD_REGISTERS
//
// MessageText:
//
//  The floppy disk controller returned inconsistent results in its registers.
//
export const ERROR_FLOPPY_BAD_REGISTERS = 1125;

//
// MessageId: ERROR_DISK_RECALIBRATE_FAILED
//
// MessageText:
//
//  While accessing the hard disk, a recalibrate operation failed, even after retries.
//
export const ERROR_DISK_RECALIBRATE_FAILED = 1126;

//
// MessageId: ERROR_DISK_OPERATION_FAILED
//
// MessageText:
//
//  While accessing the hard disk, a disk operation failed even after retries.
//
export const ERROR_DISK_OPERATION_FAILED = 1127;

//
// MessageId: ERROR_DISK_RESET_FAILED
//
// MessageText:
//
//  While accessing the hard disk, a disk controller reset was needed, but even that failed.
//
export const ERROR_DISK_RESET_FAILED = 1128;

//
// MessageId: ERROR_EOM_OVERFLOW
//
// MessageText:
//
//  Physical end of tape encountered.
//
export const ERROR_EOM_OVERFLOW = 1129;

//
// MessageId: ERROR_NOT_ENOUGH_SERVER_MEMORY
//
// MessageText:
//
//  Not enough server storage is available to process this command.
//
export const ERROR_NOT_ENOUGH_SERVER_MEMORY = 1130;

//
// MessageId: ERROR_POSSIBLE_DEADLOCK
//
// MessageText:
//
//  A potential deadlock condition has been detected.
//
export const ERROR_POSSIBLE_DEADLOCK = 1131;

//
// MessageId: ERROR_MAPPED_ALIGNMENT
//
// MessageText:
//
//  The base address or the file offset specified does not have the proper alignment.
//
export const ERROR_MAPPED_ALIGNMENT = 1132;

//
// MessageId: ERROR_SET_POWER_STATE_VETOED
//
// MessageText:
//
//  An attempt to change the system power state was vetoed by another application or driver.
//
export const ERROR_SET_POWER_STATE_VETOED = 1140;

//
// MessageId: ERROR_SET_POWER_STATE_FAILED
//
// MessageText:
//
//  The system BIOS failed an attempt to change the system power state.
//
export const ERROR_SET_POWER_STATE_FAILED = 1141;

//
// MessageId: ERROR_TOO_MANY_LINKS
//
// MessageText:
//
//  An attempt was made to create more links on a file than the file system supports.
//
export const ERROR_TOO_MANY_LINKS = 1142;

//
// MessageId: ERROR_OLD_WIN_VERSION
//
// MessageText:
//
//  The specified program requires a newer version of Windows.
//
export const ERROR_OLD_WIN_VERSION = 1150;

//
// MessageId: ERROR_APP_WRONG_OS
//
// MessageText:
//
//  The specified program is not a Windows or MS-DOS program.
//
export const ERROR_APP_WRONG_OS = 1151;

//
// MessageId: ERROR_SINGLE_INSTANCE_APP
//
// MessageText:
//
//  Cannot start more than one instance of the specified program.
//
export const ERROR_SINGLE_INSTANCE_APP = 1152;

//
// MessageId: ERROR_RMODE_APP
//
// MessageText:
//
//  The specified program was written for an earlier version of Windows.
//
export const ERROR_RMODE_APP = 1153;

//
// MessageId: ERROR_INVALID_DLL
//
// MessageText:
//
//  One of the library files needed to run this application is damaged.
//
export const ERROR_INVALID_DLL = 1154;

//
// MessageId: ERROR_NO_ASSOCIATION
//
// MessageText:
//
//  No application is associated with the specified file for this operation.
//
export const ERROR_NO_ASSOCIATION = 1155;

//
// MessageId: ERROR_DDE_FAIL
//
// MessageText:
//
//  An error occurred in sending the command to the application.
//
export const ERROR_DDE_FAIL = 1156;

//
// MessageId: ERROR_DLL_NOT_FOUND
//
// MessageText:
//
//  One of the library files needed to run this application cannot be found.
//
export const ERROR_DLL_NOT_FOUND = 1157;

//
// MessageId: ERROR_NO_MORE_USER_HANDLES
//
// MessageText:
//
//  The current process has used all of its system allowance of handles for Window Manager objects.
//
export const ERROR_NO_MORE_USER_HANDLES = 1158;

//
// MessageId: ERROR_MESSAGE_SYNC_ONLY
//
// MessageText:
//
//  The message can be used only with synchronous operations.
//
export const ERROR_MESSAGE_SYNC_ONLY = 1159;

//
// MessageId: ERROR_SOURCE_ELEMENT_EMPTY
//
// MessageText:
//
//  The indicated source element has no media.
//
export const ERROR_SOURCE_ELEMENT_EMPTY = 1160;

//
// MessageId: ERROR_DESTINATION_ELEMENT_FULL
//
// MessageText:
//
//  The indicated destination element already contains media.
//
export const ERROR_DESTINATION_ELEMENT_FULL = 1161;

//
// MessageId: ERROR_ILLEGAL_ELEMENT_ADDRESS
//
// MessageText:
//
//  The indicated element does not exist.
//
export const ERROR_ILLEGAL_ELEMENT_ADDRESS = 1162;

//
// MessageId: ERROR_MAGAZINE_NOT_PRESENT
//
// MessageText:
//
//  The indicated element is part of a magazine that is not present.
//
export const ERROR_MAGAZINE_NOT_PRESENT = 1163;

//
// MessageId: ERROR_DEVICE_REINITIALIZATION_NEEDED
//
// MessageText:
//
//  The indicated device requires reinitialization due to hardware errors.
//
export const ERROR_DEVICE_REINITIALIZATION_NEEDED = 1164; // dderror

//
// MessageId: ERROR_DEVICE_REQUIRES_CLEANING
//
// MessageText:
//
//  The device has indicated that cleaning is required before further operations are attempted.
//
export const ERROR_DEVICE_REQUIRES_CLEANING = 1165;

//
// MessageId: ERROR_DEVICE_DOOR_OPEN
//
// MessageText:
//
//  The device has indicated that its door is open.
//
export const ERROR_DEVICE_DOOR_OPEN = 1166;

//
// MessageId: ERROR_DEVICE_NOT_CONNECTED
//
// MessageText:
//
//  The device is not connected.
//
export const ERROR_DEVICE_NOT_CONNECTED = 1167;

//
// MessageId: ERROR_NOT_FOUND
//
// MessageText:
//
//  Element not found.
//
export const ERROR_NOT_FOUND = 1168;

//
// MessageId: ERROR_NO_MATCH
//
// MessageText:
//
//  There was no match for the specified key in the index.
//
export const ERROR_NO_MATCH = 1169;

//
// MessageId: ERROR_SET_NOT_FOUND
//
// MessageText:
//
//  The property set specified does not exist on the object.
//
export const ERROR_SET_NOT_FOUND = 1170;

//
// MessageId: ERROR_POINT_NOT_FOUND
//
// MessageText:
//
//  The point passed to GetMouseMovePoints is not in the buffer.
//
export const ERROR_POINT_NOT_FOUND = 1171;

//
// MessageId: ERROR_NO_TRACKING_SERVICE
//
// MessageText:
//
//  The tracking (workstation) service is not running.
//
export const ERROR_NO_TRACKING_SERVICE = 1172;

//
// MessageId: ERROR_NO_VOLUME_ID
//
// MessageText:
//
//  The Volume ID could not be found.
//
export const ERROR_NO_VOLUME_ID = 1173;

//
// MessageId: ERROR_UNABLE_TO_REMOVE_REPLACED
//
// MessageText:
//
//  Unable to remove the file to be replaced.
//
export const ERROR_UNABLE_TO_REMOVE_REPLACED = 1175;

//
// MessageId: ERROR_UNABLE_TO_MOVE_REPLACEMENT
//
// MessageText:
//
//  Unable to move the replacement file to the file to be replaced. The file to be replaced has retained its original name.
//
export const ERROR_UNABLE_TO_MOVE_REPLACEMENT = 1176;

//
// MessageId: ERROR_UNABLE_TO_MOVE_REPLACEMENT_2
//
// MessageText:
//
//  Unable to move the replacement file to the file to be replaced. The file to be replaced has been renamed using the backup name.
//
export const ERROR_UNABLE_TO_MOVE_REPLACEMENT_2 = 1177;

//
// MessageId: ERROR_JOURNAL_DELETE_IN_PROGRESS
//
// MessageText:
//
//  The volume change journal is being deleted.
//
export const ERROR_JOURNAL_DELETE_IN_PROGRESS = 1178;

//
// MessageId: ERROR_JOURNAL_NOT_ACTIVE
//
// MessageText:
//
//  The volume change journal is not active.
//
export const ERROR_JOURNAL_NOT_ACTIVE = 1179;

//
// MessageId: ERROR_POTENTIAL_FILE_FOUND
//
// MessageText:
//
//  A file was found, but it may not be the correct file.
//
export const ERROR_POTENTIAL_FILE_FOUND = 1180;

//
// MessageId: ERROR_JOURNAL_ENTRY_DELETED
//
// MessageText:
//
//  The journal entry has been deleted from the journal.
//
export const ERROR_JOURNAL_ENTRY_DELETED = 1181;

//
// MessageId: ERROR_BAD_DEVICE
//
// MessageText:
//
//  The specified device name is invalid.
//
export const ERROR_BAD_DEVICE = 1200;

//
// MessageId: ERROR_CONNECTION_UNAVAIL
//
// MessageText:
//
//  The device is not currently connected but it is a remembered connection.
//
export const ERROR_CONNECTION_UNAVAIL = 1201;

//
// MessageId: ERROR_DEVICE_ALREADY_REMEMBERED
//
// MessageText:
//
//  The local device name has a remembered connection to another network resource.
//
export const ERROR_DEVICE_ALREADY_REMEMBERED = 1202;

//
// MessageId: ERROR_NO_NET_OR_BAD_PATH
//
// MessageText:
//
//  No network provider accepted the given network path.
//
export const ERROR_NO_NET_OR_BAD_PATH = 1203;

//
// MessageId: ERROR_BAD_PROVIDER
//
// MessageText:
//
//  The specified network provider name is invalid.
//
export const ERROR_BAD_PROVIDER = 1204;

//
// MessageId: ERROR_CANNOT_OPEN_PROFILE
//
// MessageText:
//
//  Unable to open the network connection profile.
//
export const ERROR_CANNOT_OPEN_PROFILE = 1205;

//
// MessageId: ERROR_BAD_PROFILE
//
// MessageText:
//
//  The network connection profile is corrupted.
//
export const ERROR_BAD_PROFILE = 1206;

//
// MessageId: ERROR_NOT_CONTAINER
//
// MessageText:
//
//  Cannot enumerate a noncontainer.
//
export const ERROR_NOT_CONTAINER = 1207;

//
// MessageId: ERROR_EXTENDED_ERROR
//
// MessageText:
//
//  An extended error has occurred.
//
export const ERROR_EXTENDED_ERROR = 1208;

//
// MessageId: ERROR_INVALID_GROUPNAME
//
// MessageText:
//
//  The format of the specified group name is invalid.
//
export const ERROR_INVALID_GROUPNAME = 1209;

//
// MessageId: ERROR_INVALID_COMPUTERNAME
//
// MessageText:
//
//  The format of the specified computer name is invalid.
//
export const ERROR_INVALID_COMPUTERNAME = 1210;

//
// MessageId: ERROR_INVALID_EVENTNAME
//
// MessageText:
//
//  The format of the specified event name is invalid.
//
export const ERROR_INVALID_EVENTNAME = 1211;

//
// MessageId: ERROR_INVALID_DOMAINNAME
//
// MessageText:
//
//  The format of the specified domain name is invalid.
//
export const ERROR_INVALID_DOMAINNAME = 1212;

//
// MessageId: ERROR_INVALID_SERVICENAME
//
// MessageText:
//
//  The format of the specified service name is invalid.
//
export const ERROR_INVALID_SERVICENAME = 1213;

//
// MessageId: ERROR_INVALID_NETNAME
//
// MessageText:
//
//  The format of the specified network name is invalid.
//
export const ERROR_INVALID_NETNAME = 1214;

//
// MessageId: ERROR_INVALID_SHARENAME
//
// MessageText:
//
//  The format of the specified share name is invalid.
//
export const ERROR_INVALID_SHARENAME = 1215;

//
// MessageId: ERROR_INVALID_PASSWORDNAME
//
// MessageText:
//
//  The format of the specified password is invalid.
//
export const ERROR_INVALID_PASSWORDNAME = 1216;

//
// MessageId: ERROR_INVALID_MESSAGENAME
//
// MessageText:
//
//  The format of the specified message name is invalid.
//
export const ERROR_INVALID_MESSAGENAME = 1217;

//
// MessageId: ERROR_INVALID_MESSAGEDEST
//
// MessageText:
//
//  The format of the specified message destination is invalid.
//
export const ERROR_INVALID_MESSAGEDEST = 1218;

//
// MessageId: ERROR_SESSION_CREDENTIAL_CONFLICT
//
// MessageText:
//
//  Multiple connections to a server or shared resource by the same user, using more than one user name, are not allowed. Disconnect all previous connections to the server or shared resource and try again.
//
export const ERROR_SESSION_CREDENTIAL_CONFLICT = 1219;

//
// MessageId: ERROR_REMOTE_SESSION_LIMIT_EXCEEDED
//
// MessageText:
//
//  An attempt was made to establish a session to a network server, but there are already too many sessions established to that server.
//
export const ERROR_REMOTE_SESSION_LIMIT_EXCEEDED = 1220;

//
// MessageId: ERROR_DUP_DOMAINNAME
//
// MessageText:
//
//  The workgroup or domain name is already in use by another computer on the network.
//
export const ERROR_DUP_DOMAINNAME = 1221;

//
// MessageId: ERROR_NO_NETWORK
//
// MessageText:
//
//  The network is not present or not started.
//
export const ERROR_NO_NETWORK = 1222;

//
// MessageId: ERROR_CANCELLED
//
// MessageText:
//
//  The operation was canceled by the user.
//
export const ERROR_CANCELLED = 1223;

//
// MessageId: ERROR_USER_MAPPED_FILE
//
// MessageText:
//
//  The requested operation cannot be performed on a file with a user-mapped section open.
//
export const ERROR_USER_MAPPED_FILE = 1224;

//
// MessageId: ERROR_CONNECTION_REFUSED
//
// MessageText:
//
//  The remote system refused the network connection.
//
export const ERROR_CONNECTION_REFUSED = 1225;

//
// MessageId: ERROR_GRACEFUL_DISCONNECT
//
// MessageText:
//
//  The network connection was gracefully closed.
//
export const ERROR_GRACEFUL_DISCONNECT = 1226;

//
// MessageId: ERROR_ADDRESS_ALREADY_ASSOCIATED
//
// MessageText:
//
//  The network transport endpoint already has an address associated with it.
//
export const ERROR_ADDRESS_ALREADY_ASSOCIATED = 1227;

//
// MessageId: ERROR_ADDRESS_NOT_ASSOCIATED
//
// MessageText:
//
//  An address has not yet been associated with the network endpoint.
//
export const ERROR_ADDRESS_NOT_ASSOCIATED = 1228;

//
// MessageId: ERROR_CONNECTION_INVALID
//
// MessageText:
//
//  An operation was attempted on a nonexistent network connection.
//
export const ERROR_CONNECTION_INVALID = 1229;

//
// MessageId: ERROR_CONNECTION_ACTIVE
//
// MessageText:
//
//  An invalid operation was attempted on an active network connection.
//
export const ERROR_CONNECTION_ACTIVE = 1230;

//
// MessageId: ERROR_NETWORK_UNREACHABLE
//
// MessageText:
//
//  The network location cannot be reached. For information about network troubleshooting, see Windows Help.
//
export const ERROR_NETWORK_UNREACHABLE = 1231;

//
// MessageId: ERROR_HOST_UNREACHABLE
//
// MessageText:
//
//  The network location cannot be reached. For information about network troubleshooting, see Windows Help.
//
export const ERROR_HOST_UNREACHABLE = 1232;

//
// MessageId: ERROR_PROTOCOL_UNREACHABLE
//
// MessageText:
//
//  The network location cannot be reached. For information about network troubleshooting, see Windows Help.
//
export const ERROR_PROTOCOL_UNREACHABLE = 1233;

//
// MessageId: ERROR_PORT_UNREACHABLE
//
// MessageText:
//
//  No service is operating at the destination network endpoint on the remote system.
//
export const ERROR_PORT_UNREACHABLE = 1234;

//
// MessageId: ERROR_REQUEST_ABORTED
//
// MessageText:
//
//  The request was aborted.
//
export const ERROR_REQUEST_ABORTED = 1235;

//
// MessageId: ERROR_CONNECTION_ABORTED
//
// MessageText:
//
//  The network connection was aborted by the local system.
//
export const ERROR_CONNECTION_ABORTED = 1236;

//
// MessageId: ERROR_RETRY
//
// MessageText:
//
//  The operation could not be completed. A retry should be performed.
//
export const ERROR_RETRY = 1237;

//
// MessageId: ERROR_CONNECTION_COUNT_LIMIT
//
// MessageText:
//
//  A connection to the server could not be made because the limit on the number of concurrent connections for this account has been reached.
//
export const ERROR_CONNECTION_COUNT_LIMIT = 1238;

//
// MessageId: ERROR_LOGIN_TIME_RESTRICTION
//
// MessageText:
//
//  Attempting to log in during an unauthorized time of day for this account.
//
export const ERROR_LOGIN_TIME_RESTRICTION = 1239;

//
// MessageId: ERROR_LOGIN_WKSTA_RESTRICTION
//
// MessageText:
//
//  The account is not authorized to log in from this station.
//
export const ERROR_LOGIN_WKSTA_RESTRICTION = 1240;

//
// MessageId: ERROR_INCORRECT_ADDRESS
//
// MessageText:
//
//  The network address could not be used for the operation requested.
//
export const ERROR_INCORRECT_ADDRESS = 1241;

//
// MessageId: ERROR_ALREADY_REGISTERED
//
// MessageText:
//
//  The service is already registered.
//
export const ERROR_ALREADY_REGISTERED = 1242;

//
// MessageId: ERROR_SERVICE_NOT_FOUND
//
// MessageText:
//
//  The specified service does not exist.
//
export const ERROR_SERVICE_NOT_FOUND = 1243;

//
// MessageId: ERROR_NOT_AUTHENTICATED
//
// MessageText:
//
//  The operation being requested was not performed because the user has not been authenticated.
//
export const ERROR_NOT_AUTHENTICATED = 1244;

//
// MessageId: ERROR_NOT_LOGGED_ON
//
// MessageText:
//
//  The operation being requested was not performed because the user has not logged on to the network.
//  The specified service does not exist.
//
export const ERROR_NOT_LOGGED_ON = 1245;

//
// MessageId: ERROR_CONTINUE
//
// MessageText:
//
//  Continue with work in progress.
//
export const ERROR_CONTINUE = 1246; // dderror

//
// MessageId: ERROR_ALREADY_INITIALIZED
//
// MessageText:
//
//  An attempt was made to perform an initialization operation when initialization has already been completed.
//
export const ERROR_ALREADY_INITIALIZED = 1247;

//
// MessageId: ERROR_NO_MORE_DEVICES
//
// MessageText:
//
//  No more local devices.
//
export const ERROR_NO_MORE_DEVICES = 1248; // dderror

//
// MessageId: ERROR_NO_SUCH_SITE
//
// MessageText:
//
//  The specified site does not exist.
//
export const ERROR_NO_SUCH_SITE = 1249;

//
// MessageId: ERROR_DOMAIN_CONTROLLER_EXISTS
//
// MessageText:
//
//  A domain controller with the specified name already exists.
//
export const ERROR_DOMAIN_CONTROLLER_EXISTS = 1250;

//
// MessageId: ERROR_ONLY_IF_CONNECTED
//
// MessageText:
//
//  This operation is supported only when you are connected to the server.
//
export const ERROR_ONLY_IF_CONNECTED = 1251;

//
// MessageId: ERROR_OVERRIDE_NOCHANGES
//
// MessageText:
//
//  The group policy framework should call the extension even if there are no changes.
//
export const ERROR_OVERRIDE_NOCHANGES = 1252;

//
// MessageId: ERROR_BAD_USER_PROFILE
//
// MessageText:
//
//  The specified user does not have a valid profile.
//
export const ERROR_BAD_USER_PROFILE = 1253;

//
// MessageId: ERROR_NOT_SUPPORTED_ON_SBS
//
// MessageText:
//
//  This operation is not supported on a computer running Windows Server 2003 for Small Business Server
//
export const ERROR_NOT_SUPPORTED_ON_SBS = 1254;

//
// MessageId: ERROR_SERVER_SHUTDOWN_IN_PROGRESS
//
// MessageText:
//
//  The server machine is shutting down.
//
export const ERROR_SERVER_SHUTDOWN_IN_PROGRESS = 1255;

//
// MessageId: ERROR_HOST_DOWN
//
// MessageText:
//
//  The remote system is not available. For information about network troubleshooting, see Windows Help.
//
export const ERROR_HOST_DOWN = 1256;

//
// MessageId: ERROR_NON_ACCOUNT_SID
//
// MessageText:
//
//  The security identifier provided is not from an account domain.
//
export const ERROR_NON_ACCOUNT_SID = 1257;

//
// MessageId: ERROR_NON_DOMAIN_SID
//
// MessageText:
//
//  The security identifier provided does not have a domain component.
//
export const ERROR_NON_DOMAIN_SID = 1258;

//
// MessageId: ERROR_APPHELP_BLOCK
//
// MessageText:
//
//  AppHelp dialog canceled thus preventing the application from starting.
//
export const ERROR_APPHELP_BLOCK = 1259;

//
// MessageId: ERROR_ACCESS_DISABLED_BY_POLICY
//
// MessageText:
//
//  Windows cannot open this program because it has been prevented by a software restriction policy. For more information, open Event Viewer or contact your system administrator.
//
export const ERROR_ACCESS_DISABLED_BY_POLICY = 1260;

//
// MessageId: ERROR_REG_NAT_CONSUMPTION
//
// MessageText:
//
//  A program attempt to use an invalid register value.  Normally caused by an uninitialized register. This error is Itanium specific.
//
export const ERROR_REG_NAT_CONSUMPTION = 1261;

//
// MessageId: ERROR_CSCSHARE_OFFLINE
//
// MessageText:
//
//  The share is currently offline or does not exist.
//
export const ERROR_CSCSHARE_OFFLINE = 1262;

//
// MessageId: ERROR_PKINIT_FAILURE
//
// MessageText:
//
//  The kerberos protocol encountered an error while validating the
//  KDC certificate during smartcard logon.  There is more information in the
//  system event log.
//
export const ERROR_PKINIT_FAILURE = 1263;

//
// MessageId: ERROR_SMARTCARD_SUBSYSTEM_FAILURE
//
// MessageText:
//
//  The kerberos protocol encountered an error while attempting to utilize
//  the smartcard subsystem.
//
export const ERROR_SMARTCARD_SUBSYSTEM_FAILURE = 1264;

//
// MessageId: ERROR_DOWNGRADE_DETECTED
//
// MessageText:
//
//  The system detected a possible attempt to compromise security. Please ensure that you can contact the server that authenticated you.
//
export const ERROR_DOWNGRADE_DETECTED = 1265;

//
// Do not use ID's 1266 - 1270 as the symbolicNames have been moved to SEC_E_*
//
//
// MessageId: ERROR_MACHINE_LOCKED
//
// MessageText:
//
//  The machine is locked and can not be shut down without the force option.
//
export const ERROR_MACHINE_LOCKED = 1271;

//
// MessageId: ERROR_CALLBACK_SUPPLIED_INVALID_DATA
//
// MessageText:
//
//  An application-defined callback gave invalid data when called.
//
export const ERROR_CALLBACK_SUPPLIED_INVALID_DATA = 1273;

//
// MessageId: ERROR_SYNC_FOREGROUND_REFRESH_REQUIRED
//
// MessageText:
//
//  The group policy framework should call the extension in the synchronous foreground policy refresh.
//
export const ERROR_SYNC_FOREGROUND_REFRESH_REQUIRED = 1274;

//
// MessageId: ERROR_DRIVER_BLOCKED
//
// MessageText:
//
//  This driver has been blocked from loading
//
export const ERROR_DRIVER_BLOCKED = 1275;

//
// MessageId: ERROR_INVALID_IMPORT_OF_NON_DLL
//
// MessageText:
//
//  A dynamic link library (DLL) referenced a module that was neither a DLL nor the process's executable image.
//
export const ERROR_INVALID_IMPORT_OF_NON_DLL = 1276;

//
// MessageId: ERROR_ACCESS_DISABLED_WEBBLADE
//
// MessageText:
//
//  Windows cannot open this program since it has been disabled.
//
export const ERROR_ACCESS_DISABLED_WEBBLADE = 1277;

//
// MessageId: ERROR_ACCESS_DISABLED_WEBBLADE_TAMPER
//
// MessageText:
//
//  Windows cannot open this program because the license enforcement system has been tampered with or become corrupted.
//
export const ERROR_ACCESS_DISABLED_WEBBLADE_TAMPER = 1278;

//
// MessageId: ERROR_RECOVERY_FAILURE
//
// MessageText:
//
//  A transaction recover failed.
//
export const ERROR_RECOVERY_FAILURE = 1279;

//
// MessageId: ERROR_ALREADY_FIBER
//
// MessageText:
//
//  The current thread has already been converted to a fiber.
//
export const ERROR_ALREADY_FIBER = 1280;

//
// MessageId: ERROR_ALREADY_THREAD
//
// MessageText:
//
//  The current thread has already been converted from a fiber.
//
export const ERROR_ALREADY_THREAD = 1281;

//
// MessageId: ERROR_STACK_BUFFER_OVERRUN
//
// MessageText:
//
//  The system detected an overrun of a stack-based buffer in this application.  This
//  overrun could potentially allow a malicious user to gain control of this application.
//
export const ERROR_STACK_BUFFER_OVERRUN = 1282;

//
// MessageId: ERROR_PARAMETER_QUOTA_EXCEEDED
//
// MessageText:
//
//  Data present in one of the parameters is more than the function can operate on.
//
export const ERROR_PARAMETER_QUOTA_EXCEEDED = 1283;

//
// MessageId: ERROR_DEBUGGER_INACTIVE
//
// MessageText:
//
//  An attempt to do an operation on a debug object failed because the object is in the process of being deleted.
//
export const ERROR_DEBUGGER_INACTIVE = 1284;

//
// MessageId: ERROR_DELAY_LOAD_FAILED
//
// MessageText:
//
//  An attempt to delay-load a .dll or get a function address in a delay-loaded .dll failed.
//
export const ERROR_DELAY_LOAD_FAILED = 1285;

//
// MessageId: ERROR_VDM_DISALLOWED
//
// MessageText:
//
//  %1 is a 16-bit application. You do not have permissions to execute 16-bit applications. Check your permissions with your system administrator.
//
export const ERROR_VDM_DISALLOWED = 1286;

//
// MessageId: ERROR_UNIDENTIFIED_ERROR
//
// MessageText:
//
//  Insufficient information exists to identify the cause of failure.
//
export const ERROR_UNIDENTIFIED_ERROR = 1287;

///////////////////////////
//
// Add new status codes before this point unless there is a component specific section below.
//
///////////////////////////

///////////////////////////
//                       //
// Security Status Codes //
//                       //
///////////////////////////

//
// MessageId: ERROR_NOT_ALL_ASSIGNED
//
// MessageText:
//
//  Not all privileges referenced are assigned to the caller.
//
export const ERROR_NOT_ALL_ASSIGNED = 1300;

//
// MessageId: ERROR_SOME_NOT_MAPPED
//
// MessageText:
//
//  Some mapping between account names and security IDs was not done.
//
export const ERROR_SOME_NOT_MAPPED = 1301;

//
// MessageId: ERROR_NO_QUOTAS_FOR_ACCOUNT
//
// MessageText:
//
//  No system quota limits are specifically set for this account.
//
export const ERROR_NO_QUOTAS_FOR_ACCOUNT = 1302;

//
// MessageId: ERROR_LOCAL_USER_SESSION_KEY
//
// MessageText:
//
//  No encryption key is available. A well-known encryption key was returned.
//
export const ERROR_LOCAL_USER_SESSION_KEY = 1303;

//
// MessageId: ERROR_NULL_LM_PASSWORD
//
// MessageText:
//
//  The password is too complex to be converted to a LAN Manager password. The LAN Manager password returned is a NULL string.
//
export const ERROR_NULL_LM_PASSWORD = 1304;

//
// MessageId: ERROR_UNKNOWN_REVISION
//
// MessageText:
//
//  The revision level is unknown.
//
export const ERROR_UNKNOWN_REVISION = 1305;

//
// MessageId: ERROR_REVISION_MISMATCH
//
// MessageText:
//
//  Indicates two revision levels are incompatible.
//
export const ERROR_REVISION_MISMATCH = 1306;

//
// MessageId: ERROR_INVALID_OWNER
//
// MessageText:
//
//  This security ID may not be assigned as the owner of this object.
//
export const ERROR_INVALID_OWNER = 1307;

//
// MessageId: ERROR_INVALID_PRIMARY_GROUP
//
// MessageText:
//
//  This security ID may not be assigned as the primary group of an object.
//
export const ERROR_INVALID_PRIMARY_GROUP = 1308;

//
// MessageId: ERROR_NO_IMPERSONATION_TOKEN
//
// MessageText:
//
//  An attempt has been made to operate on an impersonation token by a thread that is not currently impersonating a client.
//
export const ERROR_NO_IMPERSONATION_TOKEN = 1309;

//
// MessageId: ERROR_CANT_DISABLE_MANDATORY
//
// MessageText:
//
//  The group may not be disabled.
//
export const ERROR_CANT_DISABLE_MANDATORY = 1310;

//
// MessageId: ERROR_NO_LOGON_SERVERS
//
// MessageText:
//
//  There are currently no logon servers available to service the logon request.
//
export const ERROR_NO_LOGON_SERVERS = 1311;

//
// MessageId: ERROR_NO_SUCH_LOGON_SESSION
//
// MessageText:
//
//  A specified logon session does not exist. It may already have been terminated.
//
export const ERROR_NO_SUCH_LOGON_SESSION = 1312;

//
// MessageId: ERROR_NO_SUCH_PRIVILEGE
//
// MessageText:
//
//  A specified privilege does not exist.
//
export const ERROR_NO_SUCH_PRIVILEGE = 1313;

//
// MessageId: ERROR_PRIVILEGE_NOT_HELD
//
// MessageText:
//
//  A required privilege is not held by the client.
//
export const ERROR_PRIVILEGE_NOT_HELD = 1314;

//
// MessageId: ERROR_INVALID_ACCOUNT_NAME
//
// MessageText:
//
//  The name provided is not a properly formed account name.
//
export const ERROR_INVALID_ACCOUNT_NAME = 1315;

//
// MessageId: ERROR_USER_EXISTS
//
// MessageText:
//
//  The specified user already exists.
//
export const ERROR_USER_EXISTS = 1316;

//
// MessageId: ERROR_NO_SUCH_USER
//
// MessageText:
//
//  The specified user does not exist.
//
export const ERROR_NO_SUCH_USER = 1317;

//
// MessageId: ERROR_GROUP_EXISTS
//
// MessageText:
//
//  The specified group already exists.
//
export const ERROR_GROUP_EXISTS = 1318;

//
// MessageId: ERROR_NO_SUCH_GROUP
//
// MessageText:
//
//  The specified group does not exist.
//
export const ERROR_NO_SUCH_GROUP = 1319;

//
// MessageId: ERROR_MEMBER_IN_GROUP
//
// MessageText:
//
//  Either the specified user account is already a member of the specified group, or the specified group cannot be deleted because it contains a member.
//
export const ERROR_MEMBER_IN_GROUP = 1320;

//
// MessageId: ERROR_MEMBER_NOT_IN_GROUP
//
// MessageText:
//
//  The specified user account is not a member of the specified group account.
//
export const ERROR_MEMBER_NOT_IN_GROUP = 1321;

//
// MessageId: ERROR_LAST_ADMIN
//
// MessageText:
//
//  The last remaining administration account cannot be disabled or deleted.
//
export const ERROR_LAST_ADMIN = 1322;

//
// MessageId: ERROR_WRONG_PASSWORD
//
// MessageText:
//
//  Unable to update the password. The value provided as the current password is incorrect.
//
export const ERROR_WRONG_PASSWORD = 1323;

//
// MessageId: ERROR_ILL_FORMED_PASSWORD
//
// MessageText:
//
//  Unable to update the password. The value provided for the new password contains values that are not allowed in passwords.
//
export const ERROR_ILL_FORMED_PASSWORD = 1324;

//
// MessageId: ERROR_PASSWORD_RESTRICTION
//
// MessageText:
//
//  Unable to update the password. The value provided for the new password does not meet the length, complexity, or history requirement of the domain.
//
export const ERROR_PASSWORD_RESTRICTION = 1325;

//
// MessageId: ERROR_LOGON_FAILURE
//
// MessageText:
//
//  Logon failure: unknown user name or bad password.
//
export const ERROR_LOGON_FAILURE = 1326;

//
// MessageId: ERROR_ACCOUNT_RESTRICTION
//
// MessageText:
//
//  Logon failure: user account restriction.  Possible reasons are blank passwords not allowed, logon hour restrictions, or a policy restriction has been enforced.
//
export const ERROR_ACCOUNT_RESTRICTION = 1327;

//
// MessageId: ERROR_INVALID_LOGON_HOURS
//
// MessageText:
//
//  Logon failure: account logon time restriction violation.
//
export const ERROR_INVALID_LOGON_HOURS = 1328;

//
// MessageId: ERROR_INVALID_WORKSTATION
//
// MessageText:
//
//  Logon failure: user not allowed to log on to this computer.
//
export const ERROR_INVALID_WORKSTATION = 1329;

//
// MessageId: ERROR_PASSWORD_EXPIRED
//
// MessageText:
//
//  Logon failure: the specified account password has expired.
//
export const ERROR_PASSWORD_EXPIRED = 1330;

//
// MessageId: ERROR_ACCOUNT_DISABLED
//
// MessageText:
//
//  Logon failure: account currently disabled.
//
export const ERROR_ACCOUNT_DISABLED = 1331;

//
// MessageId: ERROR_NONE_MAPPED
//
// MessageText:
//
//  No mapping between account names and security IDs was done.
//
export const ERROR_NONE_MAPPED = 1332;

//
// MessageId: ERROR_TOO_MANY_LUIDS_REQUESTED
//
// MessageText:
//
//  Too many local user identifiers (LUIDs) were requested at one time.
//
export const ERROR_TOO_MANY_LUIDS_REQUESTED = 1333;

//
// MessageId: ERROR_LUIDS_EXHAUSTED
//
// MessageText:
//
//  No more local user identifiers (LUIDs) are available.
//
export const ERROR_LUIDS_EXHAUSTED = 1334;

//
// MessageId: ERROR_INVALID_SUB_AUTHORITY
//
// MessageText:
//
//  The subauthority part of a security ID is invalid for this particular use.
//
export const ERROR_INVALID_SUB_AUTHORITY = 1335;

//
// MessageId: ERROR_INVALID_ACL
//
// MessageText:
//
//  The access control list (ACL) structure is invalid.
//
export const ERROR_INVALID_ACL = 1336;

//
// MessageId: ERROR_INVALID_SID
//
// MessageText:
//
//  The security ID structure is invalid.
//
export const ERROR_INVALID_SID = 1337;

//
// MessageId: ERROR_INVALID_SECURITY_DESCR
//
// MessageText:
//
//  The security descriptor structure is invalid.
//
export const ERROR_INVALID_SECURITY_DESCR = 1338;

//
// MessageId: ERROR_BAD_INHERITANCE_ACL
//
// MessageText:
//
//  The inherited access control list (ACL) or access control entry (ACE) could not be built.
//
export const ERROR_BAD_INHERITANCE_ACL = 1340;

//
// MessageId: ERROR_SERVER_DISABLED
//
// MessageText:
//
//  The server is currently disabled.
//
export const ERROR_SERVER_DISABLED = 1341;

//
// MessageId: ERROR_SERVER_NOT_DISABLED
//
// MessageText:
//
//  The server is currently enabled.
//
export const ERROR_SERVER_NOT_DISABLED = 1342;

//
// MessageId: ERROR_INVALID_ID_AUTHORITY
//
// MessageText:
//
//  The value provided was an invalid value for an identifier authority.
//
export const ERROR_INVALID_ID_AUTHORITY = 1343;

//
// MessageId: ERROR_ALLOTTED_SPACE_EXCEEDED
//
// MessageText:
//
//  No more memory is available for security information updates.
//
export const ERROR_ALLOTTED_SPACE_EXCEEDED = 1344;

//
// MessageId: ERROR_INVALID_GROUP_ATTRIBUTES
//
// MessageText:
//
//  The specified attributes are invalid, or incompatible with the attributes for the group as a whole.
//
export const ERROR_INVALID_GROUP_ATTRIBUTES = 1345;

//
// MessageId: ERROR_BAD_IMPERSONATION_LEVEL
//
// MessageText:
//
//  Either a required impersonation level was not provided, or the provided impersonation level is invalid.
//
export const ERROR_BAD_IMPERSONATION_LEVEL = 1346;

//
// MessageId: ERROR_CANT_OPEN_ANONYMOUS
//
// MessageText:
//
//  Cannot open an anonymous level security token.
//
export const ERROR_CANT_OPEN_ANONYMOUS = 1347;

//
// MessageId: ERROR_BAD_VALIDATION_CLASS
//
// MessageText:
//
//  The validation information class requested was invalid.
//
export const ERROR_BAD_VALIDATION_CLASS = 1348;

//
// MessageId: ERROR_BAD_TOKEN_TYPE
//
// MessageText:
//
//  The type of the token is inappropriate for its attempted use.
//
export const ERROR_BAD_TOKEN_TYPE = 1349;

//
// MessageId: ERROR_NO_SECURITY_ON_OBJECT
//
// MessageText:
//
//  Unable to perform a security operation on an object that has no associated security.
//
export const ERROR_NO_SECURITY_ON_OBJECT = 1350;

//
// MessageId: ERROR_CANT_ACCESS_DOMAIN_INFO
//
// MessageText:
//
//  Configuration information could not be read from the domain controller, either because the machine is unavailable, or access has been denied.
//
export const ERROR_CANT_ACCESS_DOMAIN_INFO = 1351;

//
// MessageId: ERROR_INVALID_SERVER_STATE
//
// MessageText:
//
//  The security account manager (SAM) or local security authority (LSA) server was in the wrong state to perform the security operation.
//
export const ERROR_INVALID_SERVER_STATE = 1352;

//
// MessageId: ERROR_INVALID_DOMAIN_STATE
//
// MessageText:
//
//  The domain was in the wrong state to perform the security operation.
//
export const ERROR_INVALID_DOMAIN_STATE = 1353;

//
// MessageId: ERROR_INVALID_DOMAIN_ROLE
//
// MessageText:
//
//  This operation is only allowed for the Primary Domain Controller of the domain.
//
export const ERROR_INVALID_DOMAIN_ROLE = 1354;

//
// MessageId: ERROR_NO_SUCH_DOMAIN
//
// MessageText:
//
//  The specified domain either does not exist or could not be contacted.
//
export const ERROR_NO_SUCH_DOMAIN = 1355;

//
// MessageId: ERROR_DOMAIN_EXISTS
//
// MessageText:
//
//  The specified domain already exists.
//
export const ERROR_DOMAIN_EXISTS = 1356;

//
// MessageId: ERROR_DOMAIN_LIMIT_EXCEEDED
//
// MessageText:
//
//  An attempt was made to exceed the limit on the number of domains per server.
//
export const ERROR_DOMAIN_LIMIT_EXCEEDED = 1357;

//
// MessageId: ERROR_INTERNAL_DB_CORRUPTION
//
// MessageText:
//
//  Unable to complete the requested operation because of either a catastrophic media failure or a data structure corruption on the disk.
//
export const ERROR_INTERNAL_DB_CORRUPTION = 1358;

//
// MessageId: ERROR_INTERNAL_ERROR
//
// MessageText:
//
//  An internal error occurred.
//
export const ERROR_INTERNAL_ERROR = 1359;

//
// MessageId: ERROR_GENERIC_NOT_MAPPED
//
// MessageText:
//
//  Generic access types were contained in an access mask which should already be mapped to nongeneric types.
//
export const ERROR_GENERIC_NOT_MAPPED = 1360;

//
// MessageId: ERROR_BAD_DESCRIPTOR_FORMAT
//
// MessageText:
//
//  A security descriptor is not in the right format (absolute or self-relative).
//
export const ERROR_BAD_DESCRIPTOR_FORMAT = 1361;

//
// MessageId: ERROR_NOT_LOGON_PROCESS
//
// MessageText:
//
//  The requested action is restricted for use by logon processes only. The calling process has not registered as a logon process.
//
export const ERROR_NOT_LOGON_PROCESS = 1362;

//
// MessageId: ERROR_LOGON_SESSION_EXISTS
//
// MessageText:
//
//  Cannot start a new logon session with an ID that is already in use.
//
export const ERROR_LOGON_SESSION_EXISTS = 1363;

//
// MessageId: ERROR_NO_SUCH_PACKAGE
//
// MessageText:
//
//  A specified authentication package is unknown.
//
export const ERROR_NO_SUCH_PACKAGE = 1364;

//
// MessageId: ERROR_BAD_LOGON_SESSION_STATE
//
// MessageText:
//
//  The logon session is not in a state that is consistent with the requested operation.
//
export const ERROR_BAD_LOGON_SESSION_STATE = 1365;

//
// MessageId: ERROR_LOGON_SESSION_COLLISION
//
// MessageText:
//
//  The logon session ID is already in use.
//
export const ERROR_LOGON_SESSION_COLLISION = 1366;

//
// MessageId: ERROR_INVALID_LOGON_TYPE
//
// MessageText:
//
//  A logon request contained an invalid logon type value.
//
export const ERROR_INVALID_LOGON_TYPE = 1367;

//
// MessageId: ERROR_CANNOT_IMPERSONATE
//
// MessageText:
//
//  Unable to impersonate using a named pipe until data has been read from that pipe.
//
export const ERROR_CANNOT_IMPERSONATE = 1368;

//
// MessageId: ERROR_RXACT_INVALID_STATE
//
// MessageText:
//
//  The transaction state of a registry subtree is incompatible with the requested operation.
//
export const ERROR_RXACT_INVALID_STATE = 1369;

//
// MessageId: ERROR_RXACT_COMMIT_FAILURE
//
// MessageText:
//
//  An internal security database corruption has been encountered.
//
export const ERROR_RXACT_COMMIT_FAILURE = 1370;

//
// MessageId: ERROR_SPECIAL_ACCOUNT
//
// MessageText:
//
//  Cannot perform this operation on built-in accounts.
//
export const ERROR_SPECIAL_ACCOUNT = 1371;

//
// MessageId: ERROR_SPECIAL_GROUP
//
// MessageText:
//
//  Cannot perform this operation on this built-in special group.
//
export const ERROR_SPECIAL_GROUP = 1372;

//
// MessageId: ERROR_SPECIAL_USER
//
// MessageText:
//
//  Cannot perform this operation on this built-in special user.
//
export const ERROR_SPECIAL_USER = 1373;

//
// MessageId: ERROR_MEMBERS_PRIMARY_GROUP
//
// MessageText:
//
//  The user cannot be removed from a group because the group is currently the user's primary group.
//
export const ERROR_MEMBERS_PRIMARY_GROUP = 1374;

//
// MessageId: ERROR_TOKEN_ALREADY_IN_USE
//
// MessageText:
//
//  The token is already in use as a primary token.
//
export const ERROR_TOKEN_ALREADY_IN_USE = 1375;

//
// MessageId: ERROR_NO_SUCH_ALIAS
//
// MessageText:
//
//  The specified local group does not exist.
//
export const ERROR_NO_SUCH_ALIAS = 1376;

//
// MessageId: ERROR_MEMBER_NOT_IN_ALIAS
//
// MessageText:
//
//  The specified account name is not a member of the local group.
//
export const ERROR_MEMBER_NOT_IN_ALIAS = 1377;

//
// MessageId: ERROR_MEMBER_IN_ALIAS
//
// MessageText:
//
//  The specified account name is already a member of the local group.
//
export const ERROR_MEMBER_IN_ALIAS = 1378;

//
// MessageId: ERROR_ALIAS_EXISTS
//
// MessageText:
//
//  The specified local group already exists.
//
export const ERROR_ALIAS_EXISTS = 1379;

//
// MessageId: ERROR_LOGON_NOT_GRANTED
//
// MessageText:
//
//  Logon failure: the user has not been granted the requested logon type at this computer.
//
export const ERROR_LOGON_NOT_GRANTED = 1380;

//
// MessageId: ERROR_TOO_MANY_SECRETS
//
// MessageText:
//
//  The maximum number of secrets that may be stored in a single system has been exceeded.
//
export const ERROR_TOO_MANY_SECRETS = 1381;

//
// MessageId: ERROR_SECRET_TOO_LONG
//
// MessageText:
//
//  The length of a secret exceeds the maximum length allowed.
//
export const ERROR_SECRET_TOO_LONG = 1382;

//
// MessageId: ERROR_INTERNAL_DB_ERROR
//
// MessageText:
//
//  The local security authority database contains an internal inconsistency.
//
export const ERROR_INTERNAL_DB_ERROR = 1383;

//
// MessageId: ERROR_TOO_MANY_CONTEXT_IDS
//
// MessageText:
//
//  During a logon attempt, the user's security context accumulated too many security IDs.
//
export const ERROR_TOO_MANY_CONTEXT_IDS = 1384;

//
// MessageId: ERROR_LOGON_TYPE_NOT_GRANTED
//
// MessageText:
//
//  Logon failure: the user has not been granted the requested logon type at this computer.
//
export const ERROR_LOGON_TYPE_NOT_GRANTED = 1385;

//
// MessageId: ERROR_NT_CROSS_ENCRYPTION_REQUIRED
//
// MessageText:
//
//  A cross-encrypted password is necessary to change a user password.
//
export const ERROR_NT_CROSS_ENCRYPTION_REQUIRED = 1386;

//
// MessageId: ERROR_NO_SUCH_MEMBER
//
// MessageText:
//
//  A member could not be added to or removed from the local group because the member does not exist.
//
export const ERROR_NO_SUCH_MEMBER = 1387;

//
// MessageId: ERROR_INVALID_MEMBER
//
// MessageText:
//
//  A new member could not be added to a local group because the member has the wrong account type.
//
export const ERROR_INVALID_MEMBER = 1388;

//
// MessageId: ERROR_TOO_MANY_SIDS
//
// MessageText:
//
//  Too many security IDs have been specified.
//
export const ERROR_TOO_MANY_SIDS = 1389;

//
// MessageId: ERROR_LM_CROSS_ENCRYPTION_REQUIRED
//
// MessageText:
//
//  A cross-encrypted password is necessary to change this user password.
//
export const ERROR_LM_CROSS_ENCRYPTION_REQUIRED = 1390;

//
// MessageId: ERROR_NO_INHERITANCE
//
// MessageText:
//
//  Indicates an ACL contains no inheritable components.
//
export const ERROR_NO_INHERITANCE = 1391;

//
// MessageId: ERROR_FILE_CORRUPT
//
// MessageText:
//
//  The file or directory is corrupted and unreadable.
//
export const ERROR_FILE_CORRUPT = 1392;

//
// MessageId: ERROR_DISK_CORRUPT
//
// MessageText:
//
//  The disk structure is corrupted and unreadable.
//
export const ERROR_DISK_CORRUPT = 1393;

//
// MessageId: ERROR_NO_USER_SESSION_KEY
//
// MessageText:
//
//  There is no user session key for the specified logon session.
//
export const ERROR_NO_USER_SESSION_KEY = 1394;

//
// MessageId: ERROR_LICENSE_QUOTA_EXCEEDED
//
// MessageText:
//
//  The service being accessed is licensed for a particular number of connections.
//  No more connections can be made to the service at this time because there are already as many connections as the service can accept.
//
export const ERROR_LICENSE_QUOTA_EXCEEDED = 1395;

//
// MessageId: ERROR_WRONG_TARGET_NAME
//
// MessageText:
//
//  Logon Failure: The target account name is incorrect.
//
export const ERROR_WRONG_TARGET_NAME = 1396;

//
// MessageId: ERROR_MUTUAL_AUTH_FAILED
//
// MessageText:
//
//  Mutual Authentication failed. The server's password is out of date at the domain controller.
//
export const ERROR_MUTUAL_AUTH_FAILED = 1397;

//
// MessageId: ERROR_TIME_SKEW
//
// MessageText:
//
//  There is a time and/or date difference between the client and server.
//
export const ERROR_TIME_SKEW = 1398;

//
// MessageId: ERROR_CURRENT_DOMAIN_NOT_ALLOWED
//
// MessageText:
//
//  This operation can not be performed on the current domain.
//
export const ERROR_CURRENT_DOMAIN_NOT_ALLOWED = 1399;

// End of security error codes

///////////////////////////
//                       //
// WinUser Error Codes   //
//                       //
///////////////////////////

//
// MessageId: ERROR_INVALID_WINDOW_HANDLE
//
// MessageText:
//
//  Invalid window handle.
//
export const ERROR_INVALID_WINDOW_HANDLE = 1400;

//
// MessageId: ERROR_INVALID_MENU_HANDLE
//
// MessageText:
//
//  Invalid menu handle.
//
export const ERROR_INVALID_MENU_HANDLE = 1401;

//
// MessageId: ERROR_INVALID_CURSOR_HANDLE
//
// MessageText:
//
//  Invalid cursor handle.
//
export const ERROR_INVALID_CURSOR_HANDLE = 1402;

//
// MessageId: ERROR_INVALID_ACCEL_HANDLE
//
// MessageText:
//
//  Invalid accelerator table handle.
//
export const ERROR_INVALID_ACCEL_HANDLE = 1403;

//
// MessageId: ERROR_INVALID_HOOK_HANDLE
//
// MessageText:
//
//  Invalid hook handle.
//
export const ERROR_INVALID_HOOK_HANDLE = 1404;

//
// MessageId: ERROR_INVALID_DWP_HANDLE
//
// MessageText:
//
//  Invalid handle to a multiple-window position structure.
//
export const ERROR_INVALID_DWP_HANDLE = 1405;

//
// MessageId: ERROR_TLW_WITH_WSCHILD
//
// MessageText:
//
//  Cannot create a top-level child window.
//
export const ERROR_TLW_WITH_WSCHILD = 1406;

//
// MessageId: ERROR_CANNOT_FIND_WND_CLASS
//
// MessageText:
//
//  Cannot find window class.
//
export const ERROR_CANNOT_FIND_WND_CLASS = 1407;

//
// MessageId: ERROR_WINDOW_OF_OTHER_THREAD
//
// MessageText:
//
//  Invalid window; it belongs to other thread.
//
export const ERROR_WINDOW_OF_OTHER_THREAD = 1408;

//
// MessageId: ERROR_HOTKEY_ALREADY_REGISTERED
//
// MessageText:
//
//  Hot key is already registered.
//
export const ERROR_HOTKEY_ALREADY_REGISTERED = 1409;

//
// MessageId: ERROR_CLASS_ALREADY_EXISTS
//
// MessageText:
//
//  Class already exists.
//
export const ERROR_CLASS_ALREADY_EXISTS = 1410;

//
// MessageId: ERROR_CLASS_DOES_NOT_EXIST
//
// MessageText:
//
//  Class does not exist.
//
export const ERROR_CLASS_DOES_NOT_EXIST = 1411;

//
// MessageId: ERROR_CLASS_HAS_WINDOWS
//
// MessageText:
//
//  Class still has open windows.
//
export const ERROR_CLASS_HAS_WINDOWS = 1412;

//
// MessageId: ERROR_INVALID_INDEX
//
// MessageText:
//
//  Invalid index.
//
export const ERROR_INVALID_INDEX = 1413;

//
// MessageId: ERROR_INVALID_ICON_HANDLE
//
// MessageText:
//
//  Invalid icon handle.
//
export const ERROR_INVALID_ICON_HANDLE = 1414;

//
// MessageId: ERROR_PRIVATE_DIALOG_INDEX
//
// MessageText:
//
//  Using private DIALOG window words.
//
export const ERROR_PRIVATE_DIALOG_INDEX = 1415;

//
// MessageId: ERROR_LISTBOX_ID_NOT_FOUND
//
// MessageText:
//
//  The list box identifier was not found.
//
export const ERROR_LISTBOX_ID_NOT_FOUND = 1416;

//
// MessageId: ERROR_NO_WILDCARD_CHARACTERS
//
// MessageText:
//
//  No wildcards were found.
//
export const ERROR_NO_WILDCARD_CHARACTERS = 1417;

//
// MessageId: ERROR_CLIPBOARD_NOT_OPEN
//
// MessageText:
//
//  Thread does not have a clipboard open.
//
export const ERROR_CLIPBOARD_NOT_OPEN = 1418;

//
// MessageId: ERROR_HOTKEY_NOT_REGISTERED
//
// MessageText:
//
//  Hot key is not registered.
//
export const ERROR_HOTKEY_NOT_REGISTERED = 1419;

//
// MessageId: ERROR_WINDOW_NOT_DIALOG
//
// MessageText:
//
//  The window is not a valid dialog window.
//
export const ERROR_WINDOW_NOT_DIALOG = 1420;

//
// MessageId: ERROR_CONTROL_ID_NOT_FOUND
//
// MessageText:
//
//  Control ID not found.
//
export const ERROR_CONTROL_ID_NOT_FOUND = 1421;

//
// MessageId: ERROR_INVALID_COMBOBOX_MESSAGE
//
// MessageText:
//
//  Invalid message for a combo box because it does not have an edit control.
//
export const ERROR_INVALID_COMBOBOX_MESSAGE = 1422;

//
// MessageId: ERROR_WINDOW_NOT_COMBOBOX
//
// MessageText:
//
//  The window is not a combo box.
//
export const ERROR_WINDOW_NOT_COMBOBOX = 1423;

//
// MessageId: ERROR_INVALID_EDIT_HEIGHT
//
// MessageText:
//
//  Height must be less than 256.
//
export const ERROR_INVALID_EDIT_HEIGHT = 1424;

//
// MessageId: ERROR_DC_NOT_FOUND
//
// MessageText:
//
//  Invalid device context (DC) handle.
//
export const ERROR_DC_NOT_FOUND = 1425;

//
// MessageId: ERROR_INVALID_HOOK_FILTER
//
// MessageText:
//
//  Invalid hook procedure type.
//
export const ERROR_INVALID_HOOK_FILTER = 1426;

//
// MessageId: ERROR_INVALID_FILTER_PROC
//
// MessageText:
//
//  Invalid hook procedure.
//
export const ERROR_INVALID_FILTER_PROC = 1427;

//
// MessageId: ERROR_HOOK_NEEDS_HMOD
//
// MessageText:
//
//  Cannot set nonlocal hook without a module handle.
//
export const ERROR_HOOK_NEEDS_HMOD = 1428;

//
// MessageId: ERROR_GLOBAL_ONLY_HOOK
//
// MessageText:
//
//  This hook procedure can only be set globally.
//
export const ERROR_GLOBAL_ONLY_HOOK = 1429;

//
// MessageId: ERROR_JOURNAL_HOOK_SET
//
// MessageText:
//
//  The journal hook procedure is already installed.
//
export const ERROR_JOURNAL_HOOK_SET = 1430;

//
// MessageId: ERROR_HOOK_NOT_INSTALLED
//
// MessageText:
//
//  The hook procedure is not installed.
//
export const ERROR_HOOK_NOT_INSTALLED = 1431;

//
// MessageId: ERROR_INVALID_LB_MESSAGE
//
// MessageText:
//
//  Invalid message for single-selection list box.
//
export const ERROR_INVALID_LB_MESSAGE = 1432;

//
// MessageId: ERROR_SETCOUNT_ON_BAD_LB
//
// MessageText:
//
//  LB_SETCOUNT sent to non-lazy list box.
//
export const ERROR_SETCOUNT_ON_BAD_LB = 1433;

//
// MessageId: ERROR_LB_WITHOUT_TABSTOPS
//
// MessageText:
//
//  This list box does not support tab stops.
//
export const ERROR_LB_WITHOUT_TABSTOPS = 1434;

//
// MessageId: ERROR_DESTROY_OBJECT_OF_OTHER_THREAD
//
// MessageText:
//
//  Cannot destroy object created by another thread.
//
export const ERROR_DESTROY_OBJECT_OF_OTHER_THREAD = 1435;

//
// MessageId: ERROR_CHILD_WINDOW_MENU
//
// MessageText:
//
//  Child windows cannot have menus.
//
export const ERROR_CHILD_WINDOW_MENU = 1436;

//
// MessageId: ERROR_NO_SYSTEM_MENU
//
// MessageText:
//
//  The window does not have a system menu.
//
export const ERROR_NO_SYSTEM_MENU = 1437;

//
// MessageId: ERROR_INVALID_MSGBOX_STYLE
//
// MessageText:
//
//  Invalid message box style.
//
export const ERROR_INVALID_MSGBOX_STYLE = 1438;

//
// MessageId: ERROR_INVALID_SPI_VALUE
//
// MessageText:
//
//  Invalid system-wide (SPI_*) parameter.
//
export const ERROR_INVALID_SPI_VALUE = 1439;

//
// MessageId: ERROR_SCREEN_ALREADY_LOCKED
//
// MessageText:
//
//  Screen already locked.
//
export const ERROR_SCREEN_ALREADY_LOCKED = 1440;

//
// MessageId: ERROR_HWNDS_HAVE_DIFF_PARENT
//
// MessageText:
//
//  All handles to windows in a multiple-window position structure must have the same parent.
//
export const ERROR_HWNDS_HAVE_DIFF_PARENT = 1441;

//
// MessageId: ERROR_NOT_CHILD_WINDOW
//
// MessageText:
//
//  The window is not a child window.
//
export const ERROR_NOT_CHILD_WINDOW = 1442;

//
// MessageId: ERROR_INVALID_GW_COMMAND
//
// MessageText:
//
//  Invalid GW_* command.
//
export const ERROR_INVALID_GW_COMMAND = 1443;

//
// MessageId: ERROR_INVALID_THREAD_ID
//
// MessageText:
//
//  Invalid thread identifier.
//
export const ERROR_INVALID_THREAD_ID = 1444;

//
// MessageId: ERROR_NON_MDICHILD_WINDOW
//
// MessageText:
//
//  Cannot process a message from a window that is not a multiple document interface (MDI) window.
//
export const ERROR_NON_MDICHILD_WINDOW = 1445;

//
// MessageId: ERROR_POPUP_ALREADY_ACTIVE
//
// MessageText:
//
//  Popup menu already active.
//
export const ERROR_POPUP_ALREADY_ACTIVE = 1446;

//
// MessageId: ERROR_NO_SCROLLBARS
//
// MessageText:
//
//  The window does not have scroll bars.
//
export const ERROR_NO_SCROLLBARS = 1447;

//
// MessageId: ERROR_INVALID_SCROLLBAR_RANGE
//
// MessageText:
//
//  Scroll bar range cannot be greater than MAXLONG.
//
export const ERROR_INVALID_SCROLLBAR_RANGE = 1448;

//
// MessageId: ERROR_INVALID_SHOWWIN_COMMAND
//
// MessageText:
//
//  Cannot show or remove the window in the way specified.
//
export const ERROR_INVALID_SHOWWIN_COMMAND = 1449;

//
// MessageId: ERROR_NO_SYSTEM_RESOURCES
//
// MessageText:
//
//  Insufficient system resources exist to complete the requested service.
//
export const ERROR_NO_SYSTEM_RESOURCES = 1450;

//
// MessageId: ERROR_NONPAGED_SYSTEM_RESOURCES
//
// MessageText:
//
//  Insufficient system resources exist to complete the requested service.
//
export const ERROR_NONPAGED_SYSTEM_RESOURCES = 1451;

//
// MessageId: ERROR_PAGED_SYSTEM_RESOURCES
//
// MessageText:
//
//  Insufficient system resources exist to complete the requested service.
//
export const ERROR_PAGED_SYSTEM_RESOURCES = 1452;

//
// MessageId: ERROR_WORKING_SET_QUOTA
//
// MessageText:
//
//  Insufficient quota to complete the requested service.
//
export const ERROR_WORKING_SET_QUOTA = 1453;

//
// MessageId: ERROR_PAGEFILE_QUOTA
//
// MessageText:
//
//  Insufficient quota to complete the requested service.
//
export const ERROR_PAGEFILE_QUOTA = 1454;

//
// MessageId: ERROR_COMMITMENT_LIMIT
//
// MessageText:
//
//  The paging file is too small for this operation to complete.
//
export const ERROR_COMMITMENT_LIMIT = 1455;

//
// MessageId: ERROR_MENU_ITEM_NOT_FOUND
//
// MessageText:
//
//  A menu item was not found.
//
export const ERROR_MENU_ITEM_NOT_FOUND = 1456;

//
// MessageId: ERROR_INVALID_KEYBOARD_HANDLE
//
// MessageText:
//
//  Invalid keyboard layout handle.
//
export const ERROR_INVALID_KEYBOARD_HANDLE = 1457;

//
// MessageId: ERROR_HOOK_TYPE_NOT_ALLOWED
//
// MessageText:
//
//  Hook type not allowed.
//
export const ERROR_HOOK_TYPE_NOT_ALLOWED = 1458;

//
// MessageId: ERROR_REQUIRES_INTERACTIVE_WINDOWSTATION
//
// MessageText:
//
//  This operation requires an interactive window station.
//
export const ERROR_REQUIRES_INTERACTIVE_WINDOWSTATION = 1459;

//
// MessageId: ERROR_TIMEOUT
//
// MessageText:
//
//  This operation returned because the timeout period expired.
//
export const ERROR_TIMEOUT = 1460;

//
// MessageId: ERROR_INVALID_MONITOR_HANDLE
//
// MessageText:
//
//  Invalid monitor handle.
//
export const ERROR_INVALID_MONITOR_HANDLE = 1461;

//
// MessageId: ERROR_INCORRECT_SIZE
//
// MessageText:
//
//  Incorrect size argument.
//
export const ERROR_INCORRECT_SIZE = 1462;

// End of WinUser error codes

///////////////////////////
//                       //
// Eventlog Status Codes //
//                       //
///////////////////////////

//
// MessageId: ERROR_EVENTLOG_FILE_CORRUPT
//
// MessageText:
//
//  The event log file is corrupted.
//
export const ERROR_EVENTLOG_FILE_CORRUPT = 1500;

//
// MessageId: ERROR_EVENTLOG_CANT_START
//
// MessageText:
//
//  No event log file could be opened, so the event logging service did not start.
//
export const ERROR_EVENTLOG_CANT_START = 1501;

//
// MessageId: ERROR_LOG_FILE_FULL
//
// MessageText:
//
//  The event log file is full.
//
export const ERROR_LOG_FILE_FULL = 1502;

//
// MessageId: ERROR_EVENTLOG_FILE_CHANGED
//
// MessageText:
//
//  The event log file has changed between read operations.
//
export const ERROR_EVENTLOG_FILE_CHANGED = 1503;

// End of eventlog error codes

///////////////////////////
//                       //
// MSI Error Codes       //
//                       //
///////////////////////////

//
// MessageId: ERROR_INSTALL_SERVICE_FAILURE
//
// MessageText:
//
//  The Windows Installer Service could not be accessed. This can occur if you are running Windows in safe mode, or if the Windows Installer is not correctly installed. Contact your support personnel for assistance.
//
export const ERROR_INSTALL_SERVICE_FAILURE = 1601;

//
// MessageId: ERROR_INSTALL_USEREXIT
//
// MessageText:
//
//  User cancelled installation.
//
export const ERROR_INSTALL_USEREXIT = 1602;

//
// MessageId: ERROR_INSTALL_FAILURE
//
// MessageText:
//
//  Fatal error during installation.
//
export const ERROR_INSTALL_FAILURE = 1603;

//
// MessageId: ERROR_INSTALL_SUSPEND
//
// MessageText:
//
//  Installation suspended, incomplete.
//
export const ERROR_INSTALL_SUSPEND = 1604;

//
// MessageId: ERROR_UNKNOWN_PRODUCT
//
// MessageText:
//
//  This action is only valid for products that are currently installed.
//
export const ERROR_UNKNOWN_PRODUCT = 1605;

//
// MessageId: ERROR_UNKNOWN_FEATURE
//
// MessageText:
//
//  Feature ID not registered.
//
export const ERROR_UNKNOWN_FEATURE = 1606;

//
// MessageId: ERROR_UNKNOWN_COMPONENT
//
// MessageText:
//
//  Component ID not registered.
//
export const ERROR_UNKNOWN_COMPONENT = 1607;

//
// MessageId: ERROR_UNKNOWN_PROPERTY
//
// MessageText:
//
//  Unknown property.
//
export const ERROR_UNKNOWN_PROPERTY = 1608;

//
// MessageId: ERROR_INVALID_HANDLE_STATE
//
// MessageText:
//
//  Handle is in an invalid state.
//
export const ERROR_INVALID_HANDLE_STATE = 1609;

//
// MessageId: ERROR_BAD_CONFIGURATION
//
// MessageText:
//
//  The configuration data for this product is corrupt.  Contact your support personnel.
//
export const ERROR_BAD_CONFIGURATION = 1610;

//
// MessageId: ERROR_INDEX_ABSENT
//
// MessageText:
//
//  Component qualifier not present.
//
export const ERROR_INDEX_ABSENT = 1611;

//
// MessageId: ERROR_INSTALL_SOURCE_ABSENT
//
// MessageText:
//
//  The installation source for this product is not available.  Verify that the source exists and that you can access it.
//
export const ERROR_INSTALL_SOURCE_ABSENT = 1612;

//
// MessageId: ERROR_INSTALL_PACKAGE_VERSION
//
// MessageText:
//
//  This installation package cannot be installed by the Windows Installer service.  You must install a Windows service pack that contains a newer version of the Windows Installer service.
//
export const ERROR_INSTALL_PACKAGE_VERSION = 1613;

//
// MessageId: ERROR_PRODUCT_UNINSTALLED
//
// MessageText:
//
//  Product is uninstalled.
//
export const ERROR_PRODUCT_UNINSTALLED = 1614;

//
// MessageId: ERROR_BAD_QUERY_SYNTAX
//
// MessageText:
//
//  SQL query syntax invalid or unsupported.
//
export const ERROR_BAD_QUERY_SYNTAX = 1615;

//
// MessageId: ERROR_INVALID_FIELD
//
// MessageText:
//
//  Record field does not exist.
//
export const ERROR_INVALID_FIELD = 1616;

//
// MessageId: ERROR_DEVICE_REMOVED
//
// MessageText:
//
//  The device has been removed.
//
export const ERROR_DEVICE_REMOVED = 1617;

//
// MessageId: ERROR_INSTALL_ALREADY_RUNNING
//
// MessageText:
//
//  Another installation is already in progress.  Complete that installation before proceeding with this install.
//
export const ERROR_INSTALL_ALREADY_RUNNING = 1618;

//
// MessageId: ERROR_INSTALL_PACKAGE_OPEN_FAILED
//
// MessageText:
//
//  This installation package could not be opened.  Verify that the package exists and that you can access it, or contact the application vendor to verify that this is a valid Windows Installer package.
//
export const ERROR_INSTALL_PACKAGE_OPEN_FAILED = 1619;

//
// MessageId: ERROR_INSTALL_PACKAGE_INVALID
//
// MessageText:
//
//  This installation package could not be opened.  Contact the application vendor to verify that this is a valid Windows Installer package.
//
export const ERROR_INSTALL_PACKAGE_INVALID = 1620;

//
// MessageId: ERROR_INSTALL_UI_FAILURE
//
// MessageText:
//
//  There was an error starting the Windows Installer service user interface.  Contact your support personnel.
//
export const ERROR_INSTALL_UI_FAILURE = 1621;

//
// MessageId: ERROR_INSTALL_LOG_FAILURE
//
// MessageText:
//
//  Error opening installation log file. Verify that the specified log file location exists and that you can write to it.
//
export const ERROR_INSTALL_LOG_FAILURE = 1622;

//
// MessageId: ERROR_INSTALL_LANGUAGE_UNSUPPORTED
//
// MessageText:
//
//  The language of this installation package is not supported by your system.
//
export const ERROR_INSTALL_LANGUAGE_UNSUPPORTED = 1623;

//
// MessageId: ERROR_INSTALL_TRANSFORM_FAILURE
//
// MessageText:
//
//  Error applying transforms.  Verify that the specified transform paths are valid.
//
export const ERROR_INSTALL_TRANSFORM_FAILURE = 1624;

//
// MessageId: ERROR_INSTALL_PACKAGE_REJECTED
//
// MessageText:
//
//  This installation is forbidden by system policy.  Contact your system administrator.
//
export const ERROR_INSTALL_PACKAGE_REJECTED = 1625;

//
// MessageId: ERROR_FUNCTION_NOT_CALLED
//
// MessageText:
//
//  Function could not be executed.
//
export const ERROR_FUNCTION_NOT_CALLED = 1626;

//
// MessageId: ERROR_FUNCTION_FAILED
//
// MessageText:
//
//  Function failed during execution.
//
export const ERROR_FUNCTION_FAILED = 1627;

//
// MessageId: ERROR_INVALID_TABLE
//
// MessageText:
//
//  Invalid or unknown table specified.
//
export const ERROR_INVALID_TABLE = 1628;

//
// MessageId: ERROR_DATATYPE_MISMATCH
//
// MessageText:
//
//  Data supplied is of wrong type.
//
export const ERROR_DATATYPE_MISMATCH = 1629;

//
// MessageId: ERROR_UNSUPPORTED_TYPE
//
// MessageText:
//
//  Data of this type is not supported.
//
export const ERROR_UNSUPPORTED_TYPE = 1630;

//
// MessageId: ERROR_CREATE_FAILED
//
// MessageText:
//
//  The Windows Installer service failed to start.  Contact your support personnel.
//
export const ERROR_CREATE_FAILED = 1631;

//
// MessageId: ERROR_INSTALL_TEMP_UNWRITABLE
//
// MessageText:
//
//  The Temp folder is on a drive that is full or is inaccessible. Free up space on the drive or verify that you have write permission on the Temp folder.
//
export const ERROR_INSTALL_TEMP_UNWRITABLE = 1632;

//
// MessageId: ERROR_INSTALL_PLATFORM_UNSUPPORTED
//
// MessageText:
//
//  This installation package is not supported by this processor type. Contact your product vendor.
//
export const ERROR_INSTALL_PLATFORM_UNSUPPORTED = 1633;

//
// MessageId: ERROR_INSTALL_NOTUSED
//
// MessageText:
//
//  Component not used on this computer.
//
export const ERROR_INSTALL_NOTUSED = 1634;

//
// MessageId: ERROR_PATCH_PACKAGE_OPEN_FAILED
//
// MessageText:
//
//  This patch package could not be opened.  Verify that the patch package exists and that you can access it, or contact the application vendor to verify that this is a valid Windows Installer patch package.
//
export const ERROR_PATCH_PACKAGE_OPEN_FAILED = 1635;

//
// MessageId: ERROR_PATCH_PACKAGE_INVALID
//
// MessageText:
//
//  This patch package could not be opened.  Contact the application vendor to verify that this is a valid Windows Installer patch package.
//
export const ERROR_PATCH_PACKAGE_INVALID = 1636;

//
// MessageId: ERROR_PATCH_PACKAGE_UNSUPPORTED
//
// MessageText:
//
//  This patch package cannot be processed by the Windows Installer service.  You must install a Windows service pack that contains a newer version of the Windows Installer service.
//
export const ERROR_PATCH_PACKAGE_UNSUPPORTED = 1637;

//
// MessageId: ERROR_PRODUCT_VERSION
//
// MessageText:
//
//  Another version of this product is already installed.  Installation of this version cannot continue.  To configure or remove the existing version of this product, use Add/Remove Programs on the Control Panel.
//
export const ERROR_PRODUCT_VERSION = 1638;

//
// MessageId: ERROR_INVALID_COMMAND_LINE
//
// MessageText:
//
//  Invalid command line argument.  Consult the Windows Installer SDK for detailed command line help.
//
export const ERROR_INVALID_COMMAND_LINE = 1639;

//
// MessageId: ERROR_INSTALL_REMOTE_DISALLOWED
//
// MessageText:
//
//  Only administrators have permission to add, remove, or configure server software during a Terminal services remote session. If you want to install or configure software on the server, contact your network administrator.
//
export const ERROR_INSTALL_REMOTE_DISALLOWED = 1640;

//
// MessageId: ERROR_SUCCESS_REBOOT_INITIATED
//
// MessageText:
//
//  The requested operation completed successfully.  The system will be restarted so the changes can take effect.
//
export const ERROR_SUCCESS_REBOOT_INITIATED = 1641;

//
// MessageId: ERROR_PATCH_TARGET_NOT_FOUND
//
// MessageText:
//
//  The upgrade patch cannot be installed by the Windows Installer service because the program to be upgraded may be missing, or the upgrade patch may update a different version of the program. Verify that the program to be upgraded exists on your computer an
//  d that you have the correct upgrade patch.
//
export const ERROR_PATCH_TARGET_NOT_FOUND = 1642;

//
// MessageId: ERROR_PATCH_PACKAGE_REJECTED
//
// MessageText:
//
//  The patch package is not permitted by software restriction policy.
//
export const ERROR_PATCH_PACKAGE_REJECTED = 1643;

//
// MessageId: ERROR_INSTALL_TRANSFORM_REJECTED
//
// MessageText:
//
//  One or more customizations are not permitted by software restriction policy.
//
export const ERROR_INSTALL_TRANSFORM_REJECTED = 1644;

//
// MessageId: ERROR_INSTALL_REMOTE_PROHIBITED
//
// MessageText:
//
//  The Windows Installer does not permit installation from a Remote Desktop Connection.
//
export const ERROR_INSTALL_REMOTE_PROHIBITED = 1645;

// End of MSI error codes

///////////////////////////
//                       //
//   RPC Status Codes    //
//                       //
///////////////////////////

//
// MessageId: RPC_S_INVALID_STRING_BINDING
//
// MessageText:
//
//  The string binding is invalid.
//
export const RPC_S_INVALID_STRING_BINDING = 1700;

//
// MessageId: RPC_S_WRONG_KIND_OF_BINDING
//
// MessageText:
//
//  The binding handle is not the correct type.
//
export const RPC_S_WRONG_KIND_OF_BINDING = 1701;

//
// MessageId: RPC_S_INVALID_BINDING
//
// MessageText:
//
//  The binding handle is invalid.
//
export const RPC_S_INVALID_BINDING = 1702;

//
// MessageId: RPC_S_PROTSEQ_NOT_SUPPORTED
//
// MessageText:
//
//  The RPC protocol sequence is not supported.
//
export const RPC_S_PROTSEQ_NOT_SUPPORTED = 1703;

//
// MessageId: RPC_S_INVALID_RPC_PROTSEQ
//
// MessageText:
//
//  The RPC protocol sequence is invalid.
//
export const RPC_S_INVALID_RPC_PROTSEQ = 1704;

//
// MessageId: RPC_S_INVALID_STRING_UUID
//
// MessageText:
//
//  The string universal unique identifier (UUID) is invalid.
//
export const RPC_S_INVALID_STRING_UUID = 1705;

//
// MessageId: RPC_S_INVALID_ENDPOINT_FORMAT
//
// MessageText:
//
//  The endpoint format is invalid.
//
export const RPC_S_INVALID_ENDPOINT_FORMAT = 1706;

//
// MessageId: RPC_S_INVALID_NET_ADDR
//
// MessageText:
//
//  The network address is invalid.
//
export const RPC_S_INVALID_NET_ADDR = 1707;

//
// MessageId: RPC_S_NO_ENDPOINT_FOUND
//
// MessageText:
//
//  No endpoint was found.
//
export const RPC_S_NO_ENDPOINT_FOUND = 1708;

//
// MessageId: RPC_S_INVALID_TIMEOUT
//
// MessageText:
//
//  The timeout value is invalid.
//
export const RPC_S_INVALID_TIMEOUT = 1709;

//
// MessageId: RPC_S_OBJECT_NOT_FOUND
//
// MessageText:
//
//  The object universal unique identifier (UUID) was not found.
//
export const RPC_S_OBJECT_NOT_FOUND = 1710;

//
// MessageId: RPC_S_ALREADY_REGISTERED
//
// MessageText:
//
//  The object universal unique identifier (UUID) has already been registered.
//
export const RPC_S_ALREADY_REGISTERED = 1711;

//
// MessageId: RPC_S_TYPE_ALREADY_REGISTERED
//
// MessageText:
//
//  The type universal unique identifier (UUID) has already been registered.
//
export const RPC_S_TYPE_ALREADY_REGISTERED = 1712;

//
// MessageId: RPC_S_ALREADY_LISTENING
//
// MessageText:
//
//  The RPC server is already listening.
//
export const RPC_S_ALREADY_LISTENING = 1713;

//
// MessageId: RPC_S_NO_PROTSEQS_REGISTERED
//
// MessageText:
//
//  No protocol sequences have been registered.
//
export const RPC_S_NO_PROTSEQS_REGISTERED = 1714;

//
// MessageId: RPC_S_NOT_LISTENING
//
// MessageText:
//
//  The RPC server is not listening.
//
export const RPC_S_NOT_LISTENING = 1715;

//
// MessageId: RPC_S_UNKNOWN_MGR_TYPE
//
// MessageText:
//
//  The manager type is unknown.
//
export const RPC_S_UNKNOWN_MGR_TYPE = 1716;

//
// MessageId: RPC_S_UNKNOWN_IF
//
// MessageText:
//
//  The interface is unknown.
//
export const RPC_S_UNKNOWN_IF = 1717;

//
// MessageId: RPC_S_NO_BINDINGS
//
// MessageText:
//
//  There are no bindings.
//
export const RPC_S_NO_BINDINGS = 1718;

//
// MessageId: RPC_S_NO_PROTSEQS
//
// MessageText:
//
//  There are no protocol sequences.
//
export const RPC_S_NO_PROTSEQS = 1719;

//
// MessageId: RPC_S_CANT_CREATE_ENDPOINT
//
// MessageText:
//
//  The endpoint cannot be created.
//
export const RPC_S_CANT_CREATE_ENDPOINT = 1720;

//
// MessageId: RPC_S_OUT_OF_RESOURCES
//
// MessageText:
//
//  Not enough resources are available to complete this operation.
//
export const RPC_S_OUT_OF_RESOURCES = 1721;

//
// MessageId: RPC_S_SERVER_UNAVAILABLE
//
// MessageText:
//
//  The RPC server is unavailable.
//
export const RPC_S_SERVER_UNAVAILABLE = 1722;

//
// MessageId: RPC_S_SERVER_TOO_BUSY
//
// MessageText:
//
//  The RPC server is too busy to complete this operation.
//
export const RPC_S_SERVER_TOO_BUSY = 1723;

//
// MessageId: RPC_S_INVALID_NETWORK_OPTIONS
//
// MessageText:
//
//  The network options are invalid.
//
export const RPC_S_INVALID_NETWORK_OPTIONS = 1724;

//
// MessageId: RPC_S_NO_CALL_ACTIVE
//
// MessageText:
//
//  There are no remote procedure calls active on this thread.
//
export const RPC_S_NO_CALL_ACTIVE = 1725;

//
// MessageId: RPC_S_CALL_FAILED
//
// MessageText:
//
//  The remote procedure call failed.
//
export const RPC_S_CALL_FAILED = 1726;

//
// MessageId: RPC_S_CALL_FAILED_DNE
//
// MessageText:
//
//  The remote procedure call failed and did not execute.
//
export const RPC_S_CALL_FAILED_DNE = 1727;

//
// MessageId: RPC_S_PROTOCOL_ERROR
//
// MessageText:
//
//  A remote procedure call (RPC) protocol error occurred.
//
export const RPC_S_PROTOCOL_ERROR = 1728;

//
// MessageId: RPC_S_UNSUPPORTED_TRANS_SYN
//
// MessageText:
//
//  The transfer syntax is not supported by the RPC server.
//
export const RPC_S_UNSUPPORTED_TRANS_SYN = 1730;

//
// MessageId: RPC_S_UNSUPPORTED_TYPE
//
// MessageText:
//
//  The universal unique identifier (UUID) type is not supported.
//
export const RPC_S_UNSUPPORTED_TYPE = 1732;

//
// MessageId: RPC_S_INVALID_TAG
//
// MessageText:
//
//  The tag is invalid.
//
export const RPC_S_INVALID_TAG = 1733;

//
// MessageId: RPC_S_INVALID_BOUND
//
// MessageText:
//
//  The array bounds are invalid.
//
export const RPC_S_INVALID_BOUND = 1734;

//
// MessageId: RPC_S_NO_ENTRY_NAME
//
// MessageText:
//
//  The binding does not contain an entry name.
//
export const RPC_S_NO_ENTRY_NAME = 1735;

//
// MessageId: RPC_S_INVALID_NAME_SYNTAX
//
// MessageText:
//
//  The name syntax is invalid.
//
export const RPC_S_INVALID_NAME_SYNTAX = 1736;

//
// MessageId: RPC_S_UNSUPPORTED_NAME_SYNTAX
//
// MessageText:
//
//  The name syntax is not supported.
//
export const RPC_S_UNSUPPORTED_NAME_SYNTAX = 1737;

//
// MessageId: RPC_S_UUID_NO_ADDRESS
//
// MessageText:
//
//  No network address is available to use to export construct a universal unique identifier (UUID).
//
export const RPC_S_UUID_NO_ADDRESS = 1739;

//
// MessageId: RPC_S_DUPLICATE_ENDPOINT
//
// MessageText:
//
//  The endpoint is a duplicate.
//
export const RPC_S_DUPLICATE_ENDPOINT = 1740;

//
// MessageId: RPC_S_UNKNOWN_AUTHN_TYPE
//
// MessageText:
//
//  The authentication type is unknown.
//
export const RPC_S_UNKNOWN_AUTHN_TYPE = 1741;

//
// MessageId: RPC_S_MAX_CALLS_TOO_SMALL
//
// MessageText:
//
//  The maximum number of calls is too small.
//
export const RPC_S_MAX_CALLS_TOO_SMALL = 1742;

//
// MessageId: RPC_S_STRING_TOO_LONG
//
// MessageText:
//
//  The string is too long.
//
export const RPC_S_STRING_TOO_LONG = 1743;

//
// MessageId: RPC_S_PROTSEQ_NOT_FOUND
//
// MessageText:
//
//  The RPC protocol sequence was not found.
//
export const RPC_S_PROTSEQ_NOT_FOUND = 1744;

//
// MessageId: RPC_S_PROCNUM_OUT_OF_RANGE
//
// MessageText:
//
//  The procedure number is out of range.
//
export const RPC_S_PROCNUM_OUT_OF_RANGE = 1745;

//
// MessageId: RPC_S_BINDING_HAS_NO_AUTH
//
// MessageText:
//
//  The binding does not contain any authentication information.
//
export const RPC_S_BINDING_HAS_NO_AUTH = 1746;

//
// MessageId: RPC_S_UNKNOWN_AUTHN_SERVICE
//
// MessageText:
//
//  The authentication service is unknown.
//
export const RPC_S_UNKNOWN_AUTHN_SERVICE = 1747;

//
// MessageId: RPC_S_UNKNOWN_AUTHN_LEVEL
//
// MessageText:
//
//  The authentication level is unknown.
//
export const RPC_S_UNKNOWN_AUTHN_LEVEL = 1748;

//
// MessageId: RPC_S_INVALID_AUTH_IDENTITY
//
// MessageText:
//
//  The security context is invalid.
//
export const RPC_S_INVALID_AUTH_IDENTITY = 1749;

//
// MessageId: RPC_S_UNKNOWN_AUTHZ_SERVICE
//
// MessageText:
//
//  The authorization service is unknown.
//
export const RPC_S_UNKNOWN_AUTHZ_SERVICE = 1750;

//
// MessageId: EPT_S_INVALID_ENTRY
//
// MessageText:
//
//  The entry is invalid.
//
export const EPT_S_INVALID_ENTRY = 1751;

//
// MessageId: EPT_S_CANT_PERFORM_OP
//
// MessageText:
//
//  The server endpoint cannot perform the operation.
//
export const EPT_S_CANT_PERFORM_OP = 1752;

//
// MessageId: EPT_S_NOT_REGISTERED
//
// MessageText:
//
//  There are no more endpoints available from the endpoint mapper.
//
export const EPT_S_NOT_REGISTERED = 1753;

//
// MessageId: RPC_S_NOTHING_TO_EXPORT
//
// MessageText:
//
//  No interfaces have been exported.
//
export const RPC_S_NOTHING_TO_EXPORT = 1754;

//
// MessageId: RPC_S_INCOMPLETE_NAME
//
// MessageText:
//
//  The entry name is incomplete.
//
export const RPC_S_INCOMPLETE_NAME = 1755;

//
// MessageId: RPC_S_INVALID_VERS_OPTION
//
// MessageText:
//
//  The version option is invalid.
//
export const RPC_S_INVALID_VERS_OPTION = 1756;

//
// MessageId: RPC_S_NO_MORE_MEMBERS
//
// MessageText:
//
//  There are no more members.
//
export const RPC_S_NO_MORE_MEMBERS = 1757;

//
// MessageId: RPC_S_NOT_ALL_OBJS_UNEXPORTED
//
// MessageText:
//
//  There is nothing to unexport.
//
export const RPC_S_NOT_ALL_OBJS_UNEXPORTED = 1758;

//
// MessageId: RPC_S_INTERFACE_NOT_FOUND
//
// MessageText:
//
//  The interface was not found.
//
export const RPC_S_INTERFACE_NOT_FOUND = 1759;

//
// MessageId: RPC_S_ENTRY_ALREADY_EXISTS
//
// MessageText:
//
//  The entry already exists.
//
export const RPC_S_ENTRY_ALREADY_EXISTS = 1760;

//
// MessageId: RPC_S_ENTRY_NOT_FOUND
//
// MessageText:
//
//  The entry is not found.
//
export const RPC_S_ENTRY_NOT_FOUND = 1761;

//
// MessageId: RPC_S_NAME_SERVICE_UNAVAILABLE
//
// MessageText:
//
//  The name service is unavailable.
//
export const RPC_S_NAME_SERVICE_UNAVAILABLE = 1762;

//
// MessageId: RPC_S_INVALID_NAF_ID
//
// MessageText:
//
//  The network address family is invalid.
//
export const RPC_S_INVALID_NAF_ID = 1763;

//
// MessageId: RPC_S_CANNOT_SUPPORT
//
// MessageText:
//
//  The requested operation is not supported.
//
export const RPC_S_CANNOT_SUPPORT = 1764;

//
// MessageId: RPC_S_NO_CONTEXT_AVAILABLE
//
// MessageText:
//
//  No security context is available to allow impersonation.
//
export const RPC_S_NO_CONTEXT_AVAILABLE = 1765;

//
// MessageId: RPC_S_INTERNAL_ERROR
//
// MessageText:
//
//  An internal error occurred in a remote procedure call (RPC).
//
export const RPC_S_INTERNAL_ERROR = 1766;

//
// MessageId: RPC_S_ZERO_DIVIDE
//
// MessageText:
//
//  The RPC server attempted an integer division by zero.
//
export const RPC_S_ZERO_DIVIDE = 1767;

//
// MessageId: RPC_S_ADDRESS_ERROR
//
// MessageText:
//
//  An addressing error occurred in the RPC server.
//
export const RPC_S_ADDRESS_ERROR = 1768;

//
// MessageId: RPC_S_FP_DIV_ZERO
//
// MessageText:
//
//  A floating-point operation at the RPC server caused a division by zero.
//
export const RPC_S_FP_DIV_ZERO = 1769;

//
// MessageId: RPC_S_FP_UNDERFLOW
//
// MessageText:
//
//  A floating-point underflow occurred at the RPC server.
//
export const RPC_S_FP_UNDERFLOW = 1770;

//
// MessageId: RPC_S_FP_OVERFLOW
//
// MessageText:
//
//  A floating-point overflow occurred at the RPC server.
//
export const RPC_S_FP_OVERFLOW = 1771;

//
// MessageId: RPC_X_NO_MORE_ENTRIES
//
// MessageText:
//
//  The list of RPC servers available for the binding of auto handles has been exhausted.
//
export const RPC_X_NO_MORE_ENTRIES = 1772;

//
// MessageId: RPC_X_SS_CHAR_TRANS_OPEN_FAIL
//
// MessageText:
//
//  Unable to open the character translation table file.
//
export const RPC_X_SS_CHAR_TRANS_OPEN_FAIL = 1773;

//
// MessageId: RPC_X_SS_CHAR_TRANS_SHORT_FILE
//
// MessageText:
//
//  The file containing the character translation table has fewer than 512 bytes.
//
export const RPC_X_SS_CHAR_TRANS_SHORT_FILE = 1774;

//
// MessageId: RPC_X_SS_IN_NULL_CONTEXT
//
// MessageText:
//
//  A null context handle was passed from the client to the host during a remote procedure call.
//
export const RPC_X_SS_IN_NULL_CONTEXT = 1775;

//
// MessageId: RPC_X_SS_CONTEXT_DAMAGED
//
// MessageText:
//
//  The context handle changed during a remote procedure call.
//
export const RPC_X_SS_CONTEXT_DAMAGED = 1777;

//
// MessageId: RPC_X_SS_HANDLES_MISMATCH
//
// MessageText:
//
//  The binding handles passed to a remote procedure call do not match.
//
export const RPC_X_SS_HANDLES_MISMATCH = 1778;

//
// MessageId: RPC_X_SS_CANNOT_GET_CALL_HANDLE
//
// MessageText:
//
//  The stub is unable to get the remote procedure call handle.
//
export const RPC_X_SS_CANNOT_GET_CALL_HANDLE = 1779;

//
// MessageId: RPC_X_NULL_REF_POINTER
//
// MessageText:
//
//  A null reference pointer was passed to the stub.
//
export const RPC_X_NULL_REF_POINTER = 1780;

//
// MessageId: RPC_X_ENUM_VALUE_OUT_OF_RANGE
//
// MessageText:
//
//  The enumeration value is out of range.
//
export const RPC_X_ENUM_VALUE_OUT_OF_RANGE = 1781;

//
// MessageId: RPC_X_BYTE_COUNT_TOO_SMALL
//
// MessageText:
//
//  The byte count is too small.
//
export const RPC_X_BYTE_COUNT_TOO_SMALL = 1782;

//
// MessageId: RPC_X_BAD_STUB_DATA
//
// MessageText:
//
//  The stub received bad data.
//
export const RPC_X_BAD_STUB_DATA = 1783;

//
// MessageId: ERROR_INVALID_USER_BUFFER
//
// MessageText:
//
//  The supplied user buffer is not valid for the requested operation.
//
export const ERROR_INVALID_USER_BUFFER = 1784;

//
// MessageId: ERROR_UNRECOGNIZED_MEDIA
//
// MessageText:
//
//  The disk media is not recognized. It may not be formatted.
//
export const ERROR_UNRECOGNIZED_MEDIA = 1785;

//
// MessageId: ERROR_NO_TRUST_LSA_SECRET
//
// MessageText:
//
//  The workstation does not have a trust secret.
//
export const ERROR_NO_TRUST_LSA_SECRET = 1786;

//
// MessageId: ERROR_NO_TRUST_SAM_ACCOUNT
//
// MessageText:
//
//  The security database on the server does not have a computer account for this workstation trust relationship.
//
export const ERROR_NO_TRUST_SAM_ACCOUNT = 1787;

//
// MessageId: ERROR_TRUSTED_DOMAIN_FAILURE
//
// MessageText:
//
//  The trust relationship between the primary domain and the trusted domain failed.
//
export const ERROR_TRUSTED_DOMAIN_FAILURE = 1788;

//
// MessageId: ERROR_TRUSTED_RELATIONSHIP_FAILURE
//
// MessageText:
//
//  The trust relationship between this workstation and the primary domain failed.
//
export const ERROR_TRUSTED_RELATIONSHIP_FAILURE = 1789;

//
// MessageId: ERROR_TRUST_FAILURE
//
// MessageText:
//
//  The network logon failed.
//
export const ERROR_TRUST_FAILURE = 1790;

//
// MessageId: RPC_S_CALL_IN_PROGRESS
//
// MessageText:
//
//  A remote procedure call is already in progress for this thread.
//
export const RPC_S_CALL_IN_PROGRESS = 1791;

//
// MessageId: ERROR_NETLOGON_NOT_STARTED
//
// MessageText:
//
//  An attempt was made to logon, but the network logon service was not started.
//
export const ERROR_NETLOGON_NOT_STARTED = 1792;

//
// MessageId: ERROR_ACCOUNT_EXPIRED
//
// MessageText:
//
//  The user's account has expired.
//
export const ERROR_ACCOUNT_EXPIRED = 1793;

//
// MessageId: ERROR_REDIRECTOR_HAS_OPEN_HANDLES
//
// MessageText:
//
//  The redirector is in use and cannot be unloaded.
//
export const ERROR_REDIRECTOR_HAS_OPEN_HANDLES = 1794;

//
// MessageId: ERROR_PRINTER_DRIVER_ALREADY_INSTALLED
//
// MessageText:
//
//  The specified printer driver is already installed.
//
export const ERROR_PRINTER_DRIVER_ALREADY_INSTALLED = 1795;

//
// MessageId: ERROR_UNKNOWN_PORT
//
// MessageText:
//
//  The specified port is unknown.
//
export const ERROR_UNKNOWN_PORT = 1796;

//
// MessageId: ERROR_UNKNOWN_PRINTER_DRIVER
//
// MessageText:
//
//  The printer driver is unknown.
//
export const ERROR_UNKNOWN_PRINTER_DRIVER = 1797;

//
// MessageId: ERROR_UNKNOWN_PRINTPROCESSOR
//
// MessageText:
//
//  The print processor is unknown.
//
export const ERROR_UNKNOWN_PRINTPROCESSOR = 1798;

//
// MessageId: ERROR_INVALID_SEPARATOR_FILE
//
// MessageText:
//
//  The specified separator file is invalid.
//
export const ERROR_INVALID_SEPARATOR_FILE = 1799;

//
// MessageId: ERROR_INVALID_PRIORITY
//
// MessageText:
//
//  The specified priority is invalid.
//
export const ERROR_INVALID_PRIORITY = 1800;

//
// MessageId: ERROR_INVALID_PRINTER_NAME
//
// MessageText:
//
//  The printer name is invalid.
//
export const ERROR_INVALID_PRINTER_NAME = 1801;

//
// MessageId: ERROR_PRINTER_ALREADY_EXISTS
//
// MessageText:
//
//  The printer already exists.
//
export const ERROR_PRINTER_ALREADY_EXISTS = 1802;

//
// MessageId: ERROR_INVALID_PRINTER_COMMAND
//
// MessageText:
//
//  The printer command is invalid.
//
export const ERROR_INVALID_PRINTER_COMMAND = 1803;

//
// MessageId: ERROR_INVALID_DATATYPE
//
// MessageText:
//
//  The specified datatype is invalid.
//
export const ERROR_INVALID_DATATYPE = 1804;

//
// MessageId: ERROR_INVALID_ENVIRONMENT
//
// MessageText:
//
//  The environment specified is invalid.
//
export const ERROR_INVALID_ENVIRONMENT = 1805;

//
// MessageId: RPC_S_NO_MORE_BINDINGS
//
// MessageText:
//
//  There are no more bindings.
//
export const RPC_S_NO_MORE_BINDINGS = 1806;

//
// MessageId: ERROR_NOLOGON_INTERDOMAIN_TRUST_ACCOUNT
//
// MessageText:
//
//  The account used is an interdomain trust account. Use your global user account or local user account to access this server.
//
export const ERROR_NOLOGON_INTERDOMAIN_TRUST_ACCOUNT = 1807;

//
// MessageId: ERROR_NOLOGON_WORKSTATION_TRUST_ACCOUNT
//
// MessageText:
//
//  The account used is a computer account. Use your global user account or local user account to access this server.
//
export const ERROR_NOLOGON_WORKSTATION_TRUST_ACCOUNT = 1808;

//
// MessageId: ERROR_NOLOGON_SERVER_TRUST_ACCOUNT
//
// MessageText:
//
//  The account used is a server trust account. Use your global user account or local user account to access this server.
//
export const ERROR_NOLOGON_SERVER_TRUST_ACCOUNT = 1809;

//
// MessageId: ERROR_DOMAIN_TRUST_INCONSISTENT
//
// MessageText:
//
//  The name or security ID (SID) of the domain specified is inconsistent with the trust information for that domain.
//
export const ERROR_DOMAIN_TRUST_INCONSISTENT = 1810;

//
// MessageId: ERROR_SERVER_HAS_OPEN_HANDLES
//
// MessageText:
//
//  The server is in use and cannot be unloaded.
//
export const ERROR_SERVER_HAS_OPEN_HANDLES = 1811;

//
// MessageId: ERROR_RESOURCE_DATA_NOT_FOUND
//
// MessageText:
//
//  The specified image file did not contain a resource section.
//
export const ERROR_RESOURCE_DATA_NOT_FOUND = 1812;

//
// MessageId: ERROR_RESOURCE_TYPE_NOT_FOUND
//
// MessageText:
//
//  The specified resource type cannot be found in the image file.
//
export const ERROR_RESOURCE_TYPE_NOT_FOUND = 1813;

//
// MessageId: ERROR_RESOURCE_NAME_NOT_FOUND
//
// MessageText:
//
//  The specified resource name cannot be found in the image file.
//
export const ERROR_RESOURCE_NAME_NOT_FOUND = 1814;

//
// MessageId: ERROR_RESOURCE_LANG_NOT_FOUND
//
// MessageText:
//
//  The specified resource language ID cannot be found in the image file.
//
export const ERROR_RESOURCE_LANG_NOT_FOUND = 1815;

//
// MessageId: ERROR_NOT_ENOUGH_QUOTA
//
// MessageText:
//
//  Not enough quota is available to process this command.
//
export const ERROR_NOT_ENOUGH_QUOTA = 1816;

//
// MessageId: RPC_S_NO_INTERFACES
//
// MessageText:
//
//  No interfaces have been registered.
//
export const RPC_S_NO_INTERFACES = 1817;

//
// MessageId: RPC_S_CALL_CANCELLED
//
// MessageText:
//
//  The remote procedure call was cancelled.
//
export const RPC_S_CALL_CANCELLED = 1818;

//
// MessageId: RPC_S_BINDING_INCOMPLETE
//
// MessageText:
//
//  The binding handle does not contain all required information.
//
export const RPC_S_BINDING_INCOMPLETE = 1819;

//
// MessageId: RPC_S_COMM_FAILURE
//
// MessageText:
//
//  A communications failure occurred during a remote procedure call.
//
export const RPC_S_COMM_FAILURE = 1820;

//
// MessageId: RPC_S_UNSUPPORTED_AUTHN_LEVEL
//
// MessageText:
//
//  The requested authentication level is not supported.
//
export const RPC_S_UNSUPPORTED_AUTHN_LEVEL = 1821;

//
// MessageId: RPC_S_NO_PRINC_NAME
//
// MessageText:
//
//  No principal name registered.
//
export const RPC_S_NO_PRINC_NAME = 1822;

//
// MessageId: RPC_S_NOT_RPC_ERROR
//
// MessageText:
//
//  The error specified is not a valid Windows RPC error code.
//
export const RPC_S_NOT_RPC_ERROR = 1823;

//
// MessageId: RPC_S_UUID_LOCAL_ONLY
//
// MessageText:
//
//  A UUID that is valid only on this computer has been allocated.
//
export const RPC_S_UUID_LOCAL_ONLY = 1824;

//
// MessageId: RPC_S_SEC_PKG_ERROR
//
// MessageText:
//
//  A security package specific error occurred.
//
export const RPC_S_SEC_PKG_ERROR = 1825;

//
// MessageId: RPC_S_NOT_CANCELLED
//
// MessageText:
//
//  Thread is not canceled.
//
export const RPC_S_NOT_CANCELLED = 1826;

//
// MessageId: RPC_X_INVALID_ES_ACTION
//
// MessageText:
//
//  Invalid operation on the encoding/decoding handle.
//
export const RPC_X_INVALID_ES_ACTION = 1827;

//
// MessageId: RPC_X_WRONG_ES_VERSION
//
// MessageText:
//
//  Incompatible version of the serializing package.
//
export const RPC_X_WRONG_ES_VERSION = 1828;

//
// MessageId: RPC_X_WRONG_STUB_VERSION
//
// MessageText:
//
//  Incompatible version of the RPC stub.
//
export const RPC_X_WRONG_STUB_VERSION = 1829;

//
// MessageId: RPC_X_INVALID_PIPE_OBJECT
//
// MessageText:
//
//  The RPC pipe object is invalid or corrupted.
//
export const RPC_X_INVALID_PIPE_OBJECT = 1830;

//
// MessageId: RPC_X_WRONG_PIPE_ORDER
//
// MessageText:
//
//  An invalid operation was attempted on an RPC pipe object.
//
export const RPC_X_WRONG_PIPE_ORDER = 1831;

//
// MessageId: RPC_X_WRONG_PIPE_VERSION
//
// MessageText:
//
//  Unsupported RPC pipe version.
//
export const RPC_X_WRONG_PIPE_VERSION = 1832;

//
// MessageId: RPC_S_GROUP_MEMBER_NOT_FOUND
//
// MessageText:
//
//  The group member was not found.
//
export const RPC_S_GROUP_MEMBER_NOT_FOUND = 1898;

//
// MessageId: EPT_S_CANT_CREATE
//
// MessageText:
//
//  The endpoint mapper database entry could not be created.
//
export const EPT_S_CANT_CREATE = 1899;

//
// MessageId: RPC_S_INVALID_OBJECT
//
// MessageText:
//
//  The object universal unique identifier (UUID) is the nil UUID.
//
export const RPC_S_INVALID_OBJECT = 1900;

//
// MessageId: ERROR_INVALID_TIME
//
// MessageText:
//
//  The specified time is invalid.
//
export const ERROR_INVALID_TIME = 1901;

//
// MessageId: ERROR_INVALID_FORM_NAME
//
// MessageText:
//
//  The specified form name is invalid.
//
export const ERROR_INVALID_FORM_NAME = 1902;

//
// MessageId: ERROR_INVALID_FORM_SIZE
//
// MessageText:
//
//  The specified form size is invalid.
//
export const ERROR_INVALID_FORM_SIZE = 1903;

//
// MessageId: ERROR_ALREADY_WAITING
//
// MessageText:
//
//  The specified printer handle is already being waited on
//
export const ERROR_ALREADY_WAITING = 1904;

//
// MessageId: ERROR_PRINTER_DELETED
//
// MessageText:
//
//  The specified printer has been deleted.
//
export const ERROR_PRINTER_DELETED = 1905;

//
// MessageId: ERROR_INVALID_PRINTER_STATE
//
// MessageText:
//
//  The state of the printer is invalid.
//
export const ERROR_INVALID_PRINTER_STATE = 1906;

//
// MessageId: ERROR_PASSWORD_MUST_CHANGE
//
// MessageText:
//
//  The user's password must be changed before logging on the first time.
//
export const ERROR_PASSWORD_MUST_CHANGE = 1907;

//
// MessageId: ERROR_DOMAIN_CONTROLLER_NOT_FOUND
//
// MessageText:
//
//  Could not find the domain controller for this domain.
//
export const ERROR_DOMAIN_CONTROLLER_NOT_FOUND = 1908;

//
// MessageId: ERROR_ACCOUNT_LOCKED_OUT
//
// MessageText:
//
//  The referenced account is currently locked out and may not be logged on to.
//
export const ERROR_ACCOUNT_LOCKED_OUT = 1909;

//
// MessageId: OR_INVALID_OXID
//
// MessageText:
//
//  The object exporter specified was not found.
//
export const OR_INVALID_OXID = 1910;

//
// MessageId: OR_INVALID_OID
//
// MessageText:
//
//  The object specified was not found.
//
export const OR_INVALID_OID = 1911;

//
// MessageId: OR_INVALID_SET
//
// MessageText:
//
//  The object resolver set specified was not found.
//
export const OR_INVALID_SET = 1912;

//
// MessageId: RPC_S_SEND_INCOMPLETE
//
// MessageText:
//
//  Some data remains to be sent in the request buffer.
//
export const RPC_S_SEND_INCOMPLETE = 1913;

//
// MessageId: RPC_S_INVALID_ASYNC_HANDLE
//
// MessageText:
//
//  Invalid asynchronous remote procedure call handle.
//
export const RPC_S_INVALID_ASYNC_HANDLE = 1914;

//
// MessageId: RPC_S_INVALID_ASYNC_CALL
//
// MessageText:
//
//  Invalid asynchronous RPC call handle for this operation.
//
export const RPC_S_INVALID_ASYNC_CALL = 1915;

//
// MessageId: RPC_X_PIPE_CLOSED
//
// MessageText:
//
//  The RPC pipe object has already been closed.
//
export const RPC_X_PIPE_CLOSED = 1916;

//
// MessageId: RPC_X_PIPE_DISCIPLINE_ERROR
//
// MessageText:
//
//  The RPC call completed before all pipes were processed.
//
export const RPC_X_PIPE_DISCIPLINE_ERROR = 1917;

//
// MessageId: RPC_X_PIPE_EMPTY
//
// MessageText:
//
//  No more data is available from the RPC pipe.
//
export const RPC_X_PIPE_EMPTY = 1918;

//
// MessageId: ERROR_NO_SITENAME
//
// MessageText:
//
//  No site name is available for this machine.
//
export const ERROR_NO_SITENAME = 1919;

//
// MessageId: ERROR_CANT_ACCESS_FILE
//
// MessageText:
//
//  The file can not be accessed by the system.
//
export const ERROR_CANT_ACCESS_FILE = 1920;

//
// MessageId: ERROR_CANT_RESOLVE_FILENAME
//
// MessageText:
//
//  The name of the file cannot be resolved by the system.
//
export const ERROR_CANT_RESOLVE_FILENAME = 1921;

//
// MessageId: RPC_S_ENTRY_TYPE_MISMATCH
//
// MessageText:
//
//  The entry is not of the expected type.
//
export const RPC_S_ENTRY_TYPE_MISMATCH = 1922;

//
// MessageId: RPC_S_NOT_ALL_OBJS_EXPORTED
//
// MessageText:
//
//  Not all object UUIDs could be exported to the specified entry.
//
export const RPC_S_NOT_ALL_OBJS_EXPORTED = 1923;

//
// MessageId: RPC_S_INTERFACE_NOT_EXPORTED
//
// MessageText:
//
//  Interface could not be exported to the specified entry.
//
export const RPC_S_INTERFACE_NOT_EXPORTED = 1924;

//
// MessageId: RPC_S_PROFILE_NOT_ADDED
//
// MessageText:
//
//  The specified profile entry could not be added.
//
export const RPC_S_PROFILE_NOT_ADDED = 1925;

//
// MessageId: RPC_S_PRF_ELT_NOT_ADDED
//
// MessageText:
//
//  The specified profile element could not be added.
//
export const RPC_S_PRF_ELT_NOT_ADDED = 1926;

//
// MessageId: RPC_S_PRF_ELT_NOT_REMOVED
//
// MessageText:
//
//  The specified profile element could not be removed.
//
export const RPC_S_PRF_ELT_NOT_REMOVED = 1927;

//
// MessageId: RPC_S_GRP_ELT_NOT_ADDED
//
// MessageText:
//
//  The group element could not be added.
//
export const RPC_S_GRP_ELT_NOT_ADDED = 1928;

//
// MessageId: RPC_S_GRP_ELT_NOT_REMOVED
//
// MessageText:
//
//  The group element could not be removed.
//
export const RPC_S_GRP_ELT_NOT_REMOVED = 1929;

//
// MessageId: ERROR_KM_DRIVER_BLOCKED
//
// MessageText:
//
//  The printer driver is not compatible with a policy enabled on your computer that blocks NT 4.0 drivers.
//
export const ERROR_KM_DRIVER_BLOCKED = 1930;

//
// MessageId: ERROR_CONTEXT_EXPIRED
//
// MessageText:
//
//  The context has expired and can no longer be used.
//
export const ERROR_CONTEXT_EXPIRED = 1931;

//
// MessageId: ERROR_PER_USER_TRUST_QUOTA_EXCEEDED
//
// MessageText:
//
//  The current user's delegated trust creation quota has been exceeded.
//
export const ERROR_PER_USER_TRUST_QUOTA_EXCEEDED = 1932;

//
// MessageId: ERROR_ALL_USER_TRUST_QUOTA_EXCEEDED
//
// MessageText:
//
//  The total delegated trust creation quota has been exceeded.
//
export const ERROR_ALL_USER_TRUST_QUOTA_EXCEEDED = 1933;

//
// MessageId: ERROR_USER_DELETE_TRUST_QUOTA_EXCEEDED
//
// MessageText:
//
//  The current user's delegated trust deletion quota has been exceeded.
//
export const ERROR_USER_DELETE_TRUST_QUOTA_EXCEEDED = 1934;

//
// MessageId: ERROR_AUTHENTICATION_FIREWALL_FAILED
//
// MessageText:
//
//  Logon Failure: The machine you are logging onto is protected by an authentication firewall.  The specified account is not allowed to authenticate to the machine.
//
export const ERROR_AUTHENTICATION_FIREWALL_FAILED = 1935;

//
// MessageId: ERROR_REMOTE_PRINT_CONNECTIONS_BLOCKED
//
// MessageText:
//
//  Remote connections to the Print Spooler are blocked by a policy set on your machine.
//
export const ERROR_REMOTE_PRINT_CONNECTIONS_BLOCKED = 1936;

///////////////////////////
//                       //
//   OpenGL Error Code   //
//                       //
///////////////////////////

//
// MessageId: ERROR_INVALID_PIXEL_FORMAT
//
// MessageText:
//
//  The pixel format is invalid.
//
export const ERROR_INVALID_PIXEL_FORMAT = 2000;

//
// MessageId: ERROR_BAD_DRIVER
//
// MessageText:
//
//  The specified driver is invalid.
//
export const ERROR_BAD_DRIVER = 2001;

//
// MessageId: ERROR_INVALID_WINDOW_STYLE
//
// MessageText:
//
//  The window style or class attribute is invalid for this operation.
//
export const ERROR_INVALID_WINDOW_STYLE = 2002;

//
// MessageId: ERROR_METAFILE_NOT_SUPPORTED
//
// MessageText:
//
//  The requested metafile operation is not supported.
//
export const ERROR_METAFILE_NOT_SUPPORTED = 2003;

//
// MessageId: ERROR_TRANSFORM_NOT_SUPPORTED
//
// MessageText:
//
//  The requested transformation operation is not supported.
//
export const ERROR_TRANSFORM_NOT_SUPPORTED = 2004;

//
// MessageId: ERROR_CLIPPING_NOT_SUPPORTED
//
// MessageText:
//
//  The requested clipping operation is not supported.
//
export const ERROR_CLIPPING_NOT_SUPPORTED = 2005;

// End of OpenGL error codes

///////////////////////////////////////////
//                                       //
//   Image Color Management Error Code   //
//                                       //
///////////////////////////////////////////

//
// MessageId: ERROR_INVALID_CMM
//
// MessageText:
//
//  The specified color management module is invalid.
//
export const ERROR_INVALID_CMM = 2010;

//
// MessageId: ERROR_INVALID_PROFILE
//
// MessageText:
//
//  The specified color profile is invalid.
//
export const ERROR_INVALID_PROFILE = 2011;

//
// MessageId: ERROR_TAG_NOT_FOUND
//
// MessageText:
//
//  The specified tag was not found.
//
export const ERROR_TAG_NOT_FOUND = 2012;

//
// MessageId: ERROR_TAG_NOT_PRESENT
//
// MessageText:
//
//  A required tag is not present.
//
export const ERROR_TAG_NOT_PRESENT = 2013;

//
// MessageId: ERROR_DUPLICATE_TAG
//
// MessageText:
//
//  The specified tag is already present.
//
export const ERROR_DUPLICATE_TAG = 2014;

//
// MessageId: ERROR_PROFILE_NOT_ASSOCIATED_WITH_DEVICE
//
// MessageText:
//
//  The specified color profile is not associated with any device.
//
export const ERROR_PROFILE_NOT_ASSOCIATED_WITH_DEVICE = 2015;

//
// MessageId: ERROR_PROFILE_NOT_FOUND
//
// MessageText:
//
//  The specified color profile was not found.
//
export const ERROR_PROFILE_NOT_FOUND = 2016;

//
// MessageId: ERROR_INVALID_COLORSPACE
//
// MessageText:
//
//  The specified color space is invalid.
//
export const ERROR_INVALID_COLORSPACE = 2017;

//
// MessageId: ERROR_ICM_NOT_ENABLED
//
// MessageText:
//
//  Image Color Management is not enabled.
//
export const ERROR_ICM_NOT_ENABLED = 2018;

//
// MessageId: ERROR_DELETING_ICM_XFORM
//
// MessageText:
//
//  There was an error while deleting the color transform.
//
export const ERROR_DELETING_ICM_XFORM = 2019;

//
// MessageId: ERROR_INVALID_TRANSFORM
//
// MessageText:
//
//  The specified color transform is invalid.
//
export const ERROR_INVALID_TRANSFORM = 2020;

//
// MessageId: ERROR_COLORSPACE_MISMATCH
//
// MessageText:
//
//  The specified transform does not match the bitmap's color space.
//
export const ERROR_COLORSPACE_MISMATCH = 2021;

//
// MessageId: ERROR_INVALID_COLORINDEX
//
// MessageText:
//
//  The specified named color index is not present in the profile.
//
export const ERROR_INVALID_COLORINDEX = 2022;

///////////////////////////
//                       //
// Winnet32 Status Codes //
//                       //
// The range 2100 through 2999 is reserved for network status codes.
// See lmerr.h for a complete listing
///////////////////////////

//
// MessageId: ERROR_CONNECTED_OTHER_PASSWORD
//
// MessageText:
//
//  The network connection was made successfully, but the user had to be prompted for a password other than the one originally specified.
//
export const ERROR_CONNECTED_OTHER_PASSWORD = 2108;

//
// MessageId: ERROR_CONNECTED_OTHER_PASSWORD_DEFAULT
//
// MessageText:
//
//  The network connection was made successfully using default credentials.
//
export const ERROR_CONNECTED_OTHER_PASSWORD_DEFAULT = 2109;

//
// MessageId: ERROR_BAD_USERNAME
//
// MessageText:
//
//  The specified username is invalid.
//
export const ERROR_BAD_USERNAME = 2202;

//
// MessageId: ERROR_NOT_CONNECTED
//
// MessageText:
//
//  This network connection does not exist.
//
export const ERROR_NOT_CONNECTED = 2250;

//
// MessageId: ERROR_OPEN_FILES
//
// MessageText:
//
//  This network connection has files open or requests pending.
//
export const ERROR_OPEN_FILES = 2401;

//
// MessageId: ERROR_ACTIVE_CONNECTIONS
//
// MessageText:
//
//  Active connections still exist.
//
export const ERROR_ACTIVE_CONNECTIONS = 2402;

//
// MessageId: ERROR_DEVICE_IN_USE
//
// MessageText:
//
//  The device is in use by an active process and cannot be disconnected.
//
export const ERROR_DEVICE_IN_USE = 2404;

////////////////////////////////////
//                                //
//     Win32 Spooler Error Codes  //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_UNKNOWN_PRINT_MONITOR
//
// MessageText:
//
//  The specified print monitor is unknown.
//
export const ERROR_UNKNOWN_PRINT_MONITOR = 3000;

//
// MessageId: ERROR_PRINTER_DRIVER_IN_USE
//
// MessageText:
//
//  The specified printer driver is currently in use.
//
export const ERROR_PRINTER_DRIVER_IN_USE = 3001;

//
// MessageId: ERROR_SPOOL_FILE_NOT_FOUND
//
// MessageText:
//
//  The spool file was not found.
//
export const ERROR_SPOOL_FILE_NOT_FOUND = 3002;

//
// MessageId: ERROR_SPL_NO_STARTDOC
//
// MessageText:
//
//  A StartDocPrinter call was not issued.
//
export const ERROR_SPL_NO_STARTDOC = 3003;

//
// MessageId: ERROR_SPL_NO_ADDJOB
//
// MessageText:
//
//  An AddJob call was not issued.
//
export const ERROR_SPL_NO_ADDJOB = 3004;

//
// MessageId: ERROR_PRINT_PROCESSOR_ALREADY_INSTALLED
//
// MessageText:
//
//  The specified print processor has already been installed.
//
export const ERROR_PRINT_PROCESSOR_ALREADY_INSTALLED = 3005;

//
// MessageId: ERROR_PRINT_MONITOR_ALREADY_INSTALLED
//
// MessageText:
//
//  The specified print monitor has already been installed.
//
export const ERROR_PRINT_MONITOR_ALREADY_INSTALLED = 3006;

//
// MessageId: ERROR_INVALID_PRINT_MONITOR
//
// MessageText:
//
//  The specified print monitor does not have the required functions.
//
export const ERROR_INVALID_PRINT_MONITOR = 3007;

//
// MessageId: ERROR_PRINT_MONITOR_IN_USE
//
// MessageText:
//
//  The specified print monitor is currently in use.
//
export const ERROR_PRINT_MONITOR_IN_USE = 3008;

//
// MessageId: ERROR_PRINTER_HAS_JOBS_QUEUED
//
// MessageText:
//
//  The requested operation is not allowed when there are jobs queued to the printer.
//
export const ERROR_PRINTER_HAS_JOBS_QUEUED = 3009;

//
// MessageId: ERROR_SUCCESS_REBOOT_REQUIRED
//
// MessageText:
//
//  The requested operation is successful. Changes will not be effective until the system is rebooted.
//
export const ERROR_SUCCESS_REBOOT_REQUIRED = 3010;

//
// MessageId: ERROR_SUCCESS_RESTART_REQUIRED
//
// MessageText:
//
//  The requested operation is successful. Changes will not be effective until the service is restarted.
//
export const ERROR_SUCCESS_RESTART_REQUIRED = 3011;

//
// MessageId: ERROR_PRINTER_NOT_FOUND
//
// MessageText:
//
//  No printers were found.
//
export const ERROR_PRINTER_NOT_FOUND = 3012;

//
// MessageId: ERROR_PRINTER_DRIVER_WARNED
//
// MessageText:
//
//  The printer driver is known to be unreliable.
//
export const ERROR_PRINTER_DRIVER_WARNED = 3013;

//
// MessageId: ERROR_PRINTER_DRIVER_BLOCKED
//
// MessageText:
//
//  The printer driver is known to harm the system.
//
export const ERROR_PRINTER_DRIVER_BLOCKED = 3014;

////////////////////////////////////
//                                //
//     Wins Error Codes           //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_WINS_INTERNAL
//
// MessageText:
//
//  WINS encountered an error while processing the command.
//
export const ERROR_WINS_INTERNAL = 4000;

//
// MessageId: ERROR_CAN_NOT_DEL_LOCAL_WINS
//
// MessageText:
//
//  The local WINS can not be deleted.
//
export const ERROR_CAN_NOT_DEL_LOCAL_WINS = 4001;

//
// MessageId: ERROR_STATIC_INIT
//
// MessageText:
//
//  The importation from the file failed.
//
export const ERROR_STATIC_INIT = 4002;

//
// MessageId: ERROR_INC_BACKUP
//
// MessageText:
//
//  The backup failed. Was a full backup done before?
//
export const ERROR_INC_BACKUP = 4003;

//
// MessageId: ERROR_FULL_BACKUP
//
// MessageText:
//
//  The backup failed. Check the directory to which you are backing the database.
//
export const ERROR_FULL_BACKUP = 4004;

//
// MessageId: ERROR_REC_NON_EXISTENT
//
// MessageText:
//
//  The name does not exist in the WINS database.
//
export const ERROR_REC_NON_EXISTENT = 4005;

//
// MessageId: ERROR_RPL_NOT_ALLOWED
//
// MessageText:
//
//  Replication with a nonconfigured partner is not allowed.
//
export const ERROR_RPL_NOT_ALLOWED = 4006;

////////////////////////////////////
//                                //
//     DHCP Error Codes           //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_DHCP_ADDRESS_CONFLICT
//
// MessageText:
//
//  The DHCP client has obtained an IP address that is already in use on the network. The local interface will be disabled until the DHCP client can obtain a new address.
//
export const ERROR_DHCP_ADDRESS_CONFLICT = 4100;

////////////////////////////////////
//                                //
//     WMI Error Codes            //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_WMI_GUID_NOT_FOUND
//
// MessageText:
//
//  The GUID passed was not recognized as valid by a WMI data provider.
//
export const ERROR_WMI_GUID_NOT_FOUND = 4200;

//
// MessageId: ERROR_WMI_INSTANCE_NOT_FOUND
//
// MessageText:
//
//  The instance name passed was not recognized as valid by a WMI data provider.
//
export const ERROR_WMI_INSTANCE_NOT_FOUND = 4201;

//
// MessageId: ERROR_WMI_ITEMID_NOT_FOUND
//
// MessageText:
//
//  The data item ID passed was not recognized as valid by a WMI data provider.
//
export const ERROR_WMI_ITEMID_NOT_FOUND = 4202;

//
// MessageId: ERROR_WMI_TRY_AGAIN
//
// MessageText:
//
//  The WMI request could not be completed and should be retried.
//
export const ERROR_WMI_TRY_AGAIN = 4203;

//
// MessageId: ERROR_WMI_DP_NOT_FOUND
//
// MessageText:
//
//  The WMI data provider could not be located.
//
export const ERROR_WMI_DP_NOT_FOUND = 4204;

//
// MessageId: ERROR_WMI_UNRESOLVED_INSTANCE_REF
//
// MessageText:
//
//  The WMI data provider references an instance set that has not been registered.
//
export const ERROR_WMI_UNRESOLVED_INSTANCE_REF = 4205;

//
// MessageId: ERROR_WMI_ALREADY_ENABLED
//
// MessageText:
//
//  The WMI data block or event notification has already been enabled.
//
export const ERROR_WMI_ALREADY_ENABLED = 4206;

//
// MessageId: ERROR_WMI_GUID_DISCONNECTED
//
// MessageText:
//
//  The WMI data block is no longer available.
//
export const ERROR_WMI_GUID_DISCONNECTED = 4207;

//
// MessageId: ERROR_WMI_SERVER_UNAVAILABLE
//
// MessageText:
//
//  The WMI data service is not available.
//
export const ERROR_WMI_SERVER_UNAVAILABLE = 4208;

//
// MessageId: ERROR_WMI_DP_FAILED
//
// MessageText:
//
//  The WMI data provider failed to carry out the request.
//
export const ERROR_WMI_DP_FAILED = 4209;

//
// MessageId: ERROR_WMI_INVALID_MOF
//
// MessageText:
//
//  The WMI MOF information is not valid.
//
export const ERROR_WMI_INVALID_MOF = 4210;

//
// MessageId: ERROR_WMI_INVALID_REGINFO
//
// MessageText:
//
//  The WMI registration information is not valid.
//
export const ERROR_WMI_INVALID_REGINFO = 4211;

//
// MessageId: ERROR_WMI_ALREADY_DISABLED
//
// MessageText:
//
//  The WMI data block or event notification has already been disabled.
//
export const ERROR_WMI_ALREADY_DISABLED = 4212;

//
// MessageId: ERROR_WMI_READ_ONLY
//
// MessageText:
//
//  The WMI data item or data block is read only.
//
export const ERROR_WMI_READ_ONLY = 4213;

//
// MessageId: ERROR_WMI_SET_FAILURE
//
// MessageText:
//
//  The WMI data item or data block could not be changed.
//
export const ERROR_WMI_SET_FAILURE = 4214;

//////////////////////////////////////////
//                                      //
// NT Media Services (RSM) Error Codes  //
//                                      //
//////////////////////////////////////////
//
// MessageId: ERROR_INVALID_MEDIA
//
// MessageText:
//
//  The media identifier does not represent a valid medium.
//
export const ERROR_INVALID_MEDIA = 4300;

//
// MessageId: ERROR_INVALID_LIBRARY
//
// MessageText:
//
//  The library identifier does not represent a valid library.
//
export const ERROR_INVALID_LIBRARY = 4301;

//
// MessageId: ERROR_INVALID_MEDIA_POOL
//
// MessageText:
//
//  The media pool identifier does not represent a valid media pool.
//
export const ERROR_INVALID_MEDIA_POOL = 4302;

//
// MessageId: ERROR_DRIVE_MEDIA_MISMATCH
//
// MessageText:
//
//  The drive and medium are not compatible or exist in different libraries.
//
export const ERROR_DRIVE_MEDIA_MISMATCH = 4303;

//
// MessageId: ERROR_MEDIA_OFFLINE
//
// MessageText:
//
//  The medium currently exists in an offline library and must be online to perform this operation.
//
export const ERROR_MEDIA_OFFLINE = 4304;

//
// MessageId: ERROR_LIBRARY_OFFLINE
//
// MessageText:
//
//  The operation cannot be performed on an offline library.
//
export const ERROR_LIBRARY_OFFLINE = 4305;

//
// MessageId: ERROR_EMPTY
//
// MessageText:
//
//  The library, drive, or media pool is empty.
//
export const ERROR_EMPTY = 4306;

//
// MessageId: ERROR_NOT_EMPTY
//
// MessageText:
//
//  The library, drive, or media pool must be empty to perform this operation.
//
export const ERROR_NOT_EMPTY = 4307;

//
// MessageId: ERROR_MEDIA_UNAVAILABLE
//
// MessageText:
//
//  No media is currently available in this media pool or library.
//
export const ERROR_MEDIA_UNAVAILABLE = 4308;

//
// MessageId: ERROR_RESOURCE_DISABLED
//
// MessageText:
//
//  A resource required for this operation is disabled.
//
export const ERROR_RESOURCE_DISABLED = 4309;

//
// MessageId: ERROR_INVALID_CLEANER
//
// MessageText:
//
//  The media identifier does not represent a valid cleaner.
//
export const ERROR_INVALID_CLEANER = 4310;

//
// MessageId: ERROR_UNABLE_TO_CLEAN
//
// MessageText:
//
//  The drive cannot be cleaned or does not support cleaning.
//
export const ERROR_UNABLE_TO_CLEAN = 4311;

//
// MessageId: ERROR_OBJECT_NOT_FOUND
//
// MessageText:
//
//  The object identifier does not represent a valid object.
//
export const ERROR_OBJECT_NOT_FOUND = 4312;

//
// MessageId: ERROR_DATABASE_FAILURE
//
// MessageText:
//
//  Unable to read from or write to the database.
//
export const ERROR_DATABASE_FAILURE = 4313;

//
// MessageId: ERROR_DATABASE_FULL
//
// MessageText:
//
//  The database is full.
//
export const ERROR_DATABASE_FULL = 4314;

//
// MessageId: ERROR_MEDIA_INCOMPATIBLE
//
// MessageText:
//
//  The medium is not compatible with the device or media pool.
//
export const ERROR_MEDIA_INCOMPATIBLE = 4315;

//
// MessageId: ERROR_RESOURCE_NOT_PRESENT
//
// MessageText:
//
//  The resource required for this operation does not exist.
//
export const ERROR_RESOURCE_NOT_PRESENT = 4316;

//
// MessageId: ERROR_INVALID_OPERATION
//
// MessageText:
//
//  The operation identifier is not valid.
//
export const ERROR_INVALID_OPERATION = 4317;

//
// MessageId: ERROR_MEDIA_NOT_AVAILABLE
//
// MessageText:
//
//  The media is not mounted or ready for use.
//
export const ERROR_MEDIA_NOT_AVAILABLE = 4318;

//
// MessageId: ERROR_DEVICE_NOT_AVAILABLE
//
// MessageText:
//
//  The device is not ready for use.
//
export const ERROR_DEVICE_NOT_AVAILABLE = 4319;

//
// MessageId: ERROR_REQUEST_REFUSED
//
// MessageText:
//
//  The operator or administrator has refused the request.
//
export const ERROR_REQUEST_REFUSED = 4320;

//
// MessageId: ERROR_INVALID_DRIVE_OBJECT
//
// MessageText:
//
//  The drive identifier does not represent a valid drive.
//
export const ERROR_INVALID_DRIVE_OBJECT = 4321;

//
// MessageId: ERROR_LIBRARY_FULL
//
// MessageText:
//
//  Library is full.  No slot is available for use.
//
export const ERROR_LIBRARY_FULL = 4322;

//
// MessageId: ERROR_MEDIUM_NOT_ACCESSIBLE
//
// MessageText:
//
//  The transport cannot access the medium.
//
export const ERROR_MEDIUM_NOT_ACCESSIBLE = 4323;

//
// MessageId: ERROR_UNABLE_TO_LOAD_MEDIUM
//
// MessageText:
//
//  Unable to load the medium into the drive.
//
export const ERROR_UNABLE_TO_LOAD_MEDIUM = 4324;

//
// MessageId: ERROR_UNABLE_TO_INVENTORY_DRIVE
//
// MessageText:
//
//  Unable to retrieve the drive status.
//
export const ERROR_UNABLE_TO_INVENTORY_DRIVE = 4325;

//
// MessageId: ERROR_UNABLE_TO_INVENTORY_SLOT
//
// MessageText:
//
//  Unable to retrieve the slot status.
//
export const ERROR_UNABLE_TO_INVENTORY_SLOT = 4326;

//
// MessageId: ERROR_UNABLE_TO_INVENTORY_TRANSPORT
//
// MessageText:
//
//  Unable to retrieve status about the transport.
//
export const ERROR_UNABLE_TO_INVENTORY_TRANSPORT = 4327;

//
// MessageId: ERROR_TRANSPORT_FULL
//
// MessageText:
//
//  Cannot use the transport because it is already in use.
//
export const ERROR_TRANSPORT_FULL = 4328;

//
// MessageId: ERROR_CONTROLLING_IEPORT
//
// MessageText:
//
//  Unable to open or close the inject/eject port.
//
export const ERROR_CONTROLLING_IEPORT = 4329;

//
// MessageId: ERROR_UNABLE_TO_EJECT_MOUNTED_MEDIA
//
// MessageText:
//
//  Unable to eject the medium because it is in a drive.
//
export const ERROR_UNABLE_TO_EJECT_MOUNTED_MEDIA = 4330;

//
// MessageId: ERROR_CLEANER_SLOT_SET
//
// MessageText:
//
//  A cleaner slot is already reserved.
//
export const ERROR_CLEANER_SLOT_SET = 4331;

//
// MessageId: ERROR_CLEANER_SLOT_NOT_SET
//
// MessageText:
//
//  A cleaner slot is not reserved.
//
export const ERROR_CLEANER_SLOT_NOT_SET = 4332;

//
// MessageId: ERROR_CLEANER_CARTRIDGE_SPENT
//
// MessageText:
//
//  The cleaner cartridge has performed the maximum number of drive cleanings.
//
export const ERROR_CLEANER_CARTRIDGE_SPENT = 4333;

//
// MessageId: ERROR_UNEXPECTED_OMID
//
// MessageText:
//
//  Unexpected on-medium identifier.
//
export const ERROR_UNEXPECTED_OMID = 4334;

//
// MessageId: ERROR_CANT_DELETE_LAST_ITEM
//
// MessageText:
//
//  The last remaining item in this group or resource cannot be deleted.
//
export const ERROR_CANT_DELETE_LAST_ITEM = 4335;

//
// MessageId: ERROR_MESSAGE_EXCEEDS_MAX_SIZE
//
// MessageText:
//
//  The message provided exceeds the maximum size allowed for this parameter.
//
export const ERROR_MESSAGE_EXCEEDS_MAX_SIZE = 4336;

//
// MessageId: ERROR_VOLUME_CONTAINS_SYS_FILES
//
// MessageText:
//
//  The volume contains system or paging files.
//
export const ERROR_VOLUME_CONTAINS_SYS_FILES = 4337;

//
// MessageId: ERROR_INDIGENOUS_TYPE
//
// MessageText:
//
//  The media type cannot be removed from this library since at least one drive in the library reports it can support this media type.
//
export const ERROR_INDIGENOUS_TYPE = 4338;

//
// MessageId: ERROR_NO_SUPPORTING_DRIVES
//
// MessageText:
//
//  This offline media cannot be mounted on this system since no enabled drives are present which can be used.
//
export const ERROR_NO_SUPPORTING_DRIVES = 4339;

//
// MessageId: ERROR_CLEANER_CARTRIDGE_INSTALLED
//
// MessageText:
//
//  A cleaner cartridge is present in the tape library.
//
export const ERROR_CLEANER_CARTRIDGE_INSTALLED = 4340;

//
// MessageId: ERROR_IEPORT_FULL
//
// MessageText:
//
//  Cannot use the ieport because it is not empty.
//
export const ERROR_IEPORT_FULL = 4341;

////////////////////////////////////////////
//                                        //
// NT Remote Storage Service Error Codes  //
//                                        //
////////////////////////////////////////////
//
// MessageId: ERROR_FILE_OFFLINE
//
// MessageText:
//
//  The remote storage service was not able to recall the file.
//
export const ERROR_FILE_OFFLINE = 4350;

//
// MessageId: ERROR_REMOTE_STORAGE_NOT_ACTIVE
//
// MessageText:
//
//  The remote storage service is not operational at this time.
//
export const ERROR_REMOTE_STORAGE_NOT_ACTIVE = 4351;

//
// MessageId: ERROR_REMOTE_STORAGE_MEDIA_ERROR
//
// MessageText:
//
//  The remote storage service encountered a media error.
//
export const ERROR_REMOTE_STORAGE_MEDIA_ERROR = 4352;

////////////////////////////////////////////
//                                        //
// NT Reparse Points Error Codes          //
//                                        //
////////////////////////////////////////////
//
// MessageId: ERROR_NOT_A_REPARSE_POINT
//
// MessageText:
//
//  The file or directory is not a reparse point.
//
export const ERROR_NOT_A_REPARSE_POINT = 4390;

//
// MessageId: ERROR_REPARSE_ATTRIBUTE_CONFLICT
//
// MessageText:
//
//  The reparse point attribute cannot be set because it conflicts with an existing attribute.
//
export const ERROR_REPARSE_ATTRIBUTE_CONFLICT = 4391;

//
// MessageId: ERROR_INVALID_REPARSE_DATA
//
// MessageText:
//
//  The data present in the reparse point buffer is invalid.
//
export const ERROR_INVALID_REPARSE_DATA = 4392;

//
// MessageId: ERROR_REPARSE_TAG_INVALID
//
// MessageText:
//
//  The tag present in the reparse point buffer is invalid.
//
export const ERROR_REPARSE_TAG_INVALID = 4393;

//
// MessageId: ERROR_REPARSE_TAG_MISMATCH
//
// MessageText:
//
//  There is a mismatch between the tag specified in the request and the tag present in the reparse point.
//
//
export const ERROR_REPARSE_TAG_MISMATCH = 4394;

////////////////////////////////////////////
//                                        //
// NT Single Instance Store Error Codes   //
//                                        //
////////////////////////////////////////////
//
// MessageId: ERROR_VOLUME_NOT_SIS_ENABLED
//
// MessageText:
//
//  Single Instance Storage is not available on this volume.
//
export const ERROR_VOLUME_NOT_SIS_ENABLED = 4500;

////////////////////////////////////
//                                //
//     Cluster Error Codes        //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_DEPENDENT_RESOURCE_EXISTS
//
// MessageText:
//
//  The cluster resource cannot be moved to another group because other resources are dependent on it.
//
export const ERROR_DEPENDENT_RESOURCE_EXISTS = 5001;

//
// MessageId: ERROR_DEPENDENCY_NOT_FOUND
//
// MessageText:
//
//  The cluster resource dependency cannot be found.
//
export const ERROR_DEPENDENCY_NOT_FOUND = 5002;

//
// MessageId: ERROR_DEPENDENCY_ALREADY_EXISTS
//
// MessageText:
//
//  The cluster resource cannot be made dependent on the specified resource because it is already dependent.
//
export const ERROR_DEPENDENCY_ALREADY_EXISTS = 5003;

//
// MessageId: ERROR_RESOURCE_NOT_ONLINE
//
// MessageText:
//
//  The cluster resource is not online.
//
export const ERROR_RESOURCE_NOT_ONLINE = 5004;

//
// MessageId: ERROR_HOST_NODE_NOT_AVAILABLE
//
// MessageText:
//
//  A cluster node is not available for this operation.
//
export const ERROR_HOST_NODE_NOT_AVAILABLE = 5005;

//
// MessageId: ERROR_RESOURCE_NOT_AVAILABLE
//
// MessageText:
//
//  The cluster resource is not available.
//
export const ERROR_RESOURCE_NOT_AVAILABLE = 5006;

//
// MessageId: ERROR_RESOURCE_NOT_FOUND
//
// MessageText:
//
//  The cluster resource could not be found.
//
export const ERROR_RESOURCE_NOT_FOUND = 5007;

//
// MessageId: ERROR_SHUTDOWN_CLUSTER
//
// MessageText:
//
//  The cluster is being shut down.
//
export const ERROR_SHUTDOWN_CLUSTER = 5008;

//
// MessageId: ERROR_CANT_EVICT_ACTIVE_NODE
//
// MessageText:
//
//  A cluster node cannot be evicted from the cluster unless the node is down or it is the last node.
//
export const ERROR_CANT_EVICT_ACTIVE_NODE = 5009;

//
// MessageId: ERROR_OBJECT_ALREADY_EXISTS
//
// MessageText:
//
//  The object already exists.
//
export const ERROR_OBJECT_ALREADY_EXISTS = 5010;

//
// MessageId: ERROR_OBJECT_IN_LIST
//
// MessageText:
//
//  The object is already in the list.
//
export const ERROR_OBJECT_IN_LIST = 5011;

//
// MessageId: ERROR_GROUP_NOT_AVAILABLE
//
// MessageText:
//
//  The cluster group is not available for any new requests.
//
export const ERROR_GROUP_NOT_AVAILABLE = 5012;

//
// MessageId: ERROR_GROUP_NOT_FOUND
//
// MessageText:
//
//  The cluster group could not be found.
//
export const ERROR_GROUP_NOT_FOUND = 5013;

//
// MessageId: ERROR_GROUP_NOT_ONLINE
//
// MessageText:
//
//  The operation could not be completed because the cluster group is not online.
//
export const ERROR_GROUP_NOT_ONLINE = 5014;

//
// MessageId: ERROR_HOST_NODE_NOT_RESOURCE_OWNER
//
// MessageText:
//
//  The cluster node is not the owner of the resource.
//
export const ERROR_HOST_NODE_NOT_RESOURCE_OWNER = 5015;

//
// MessageId: ERROR_HOST_NODE_NOT_GROUP_OWNER
//
// MessageText:
//
//  The cluster node is not the owner of the group.
//
export const ERROR_HOST_NODE_NOT_GROUP_OWNER = 5016;

//
// MessageId: ERROR_RESMON_CREATE_FAILED
//
// MessageText:
//
//  The cluster resource could not be created in the specified resource monitor.
//
export const ERROR_RESMON_CREATE_FAILED = 5017;

//
// MessageId: ERROR_RESMON_ONLINE_FAILED
//
// MessageText:
//
//  The cluster resource could not be brought online by the resource monitor.
//
export const ERROR_RESMON_ONLINE_FAILED = 5018;

//
// MessageId: ERROR_RESOURCE_ONLINE
//
// MessageText:
//
//  The operation could not be completed because the cluster resource is online.
//
export const ERROR_RESOURCE_ONLINE = 5019;

//
// MessageId: ERROR_QUORUM_RESOURCE
//
// MessageText:
//
//  The cluster resource could not be deleted or brought offline because it is the quorum resource.
//
export const ERROR_QUORUM_RESOURCE = 5020;

//
// MessageId: ERROR_NOT_QUORUM_CAPABLE
//
// MessageText:
//
//  The cluster could not make the specified resource a quorum resource because it is not capable of being a quorum resource.
//
export const ERROR_NOT_QUORUM_CAPABLE = 5021;

//
// MessageId: ERROR_CLUSTER_SHUTTING_DOWN
//
// MessageText:
//
//  The cluster software is shutting down.
//
export const ERROR_CLUSTER_SHUTTING_DOWN = 5022;

//
// MessageId: ERROR_INVALID_STATE
//
// MessageText:
//
//  The group or resource is not in the correct state to perform the requested operation.
//
export const ERROR_INVALID_STATE = 5023;

//
// MessageId: ERROR_RESOURCE_PROPERTIES_STORED
//
// MessageText:
//
//  The properties were stored but not all changes will take effect until the next time the resource is brought online.
//
export const ERROR_RESOURCE_PROPERTIES_STORED = 5024;

//
// MessageId: ERROR_NOT_QUORUM_CLASS
//
// MessageText:
//
//  The cluster could not make the specified resource a quorum resource because it does not belong to a shared storage class.
//
export const ERROR_NOT_QUORUM_CLASS = 5025;

//
// MessageId: ERROR_CORE_RESOURCE
//
// MessageText:
//
//  The cluster resource could not be deleted since it is a core resource.
//
export const ERROR_CORE_RESOURCE = 5026;

//
// MessageId: ERROR_QUORUM_RESOURCE_ONLINE_FAILED
//
// MessageText:
//
//  The quorum resource failed to come online.
//
export const ERROR_QUORUM_RESOURCE_ONLINE_FAILED = 5027;

//
// MessageId: ERROR_QUORUMLOG_OPEN_FAILED
//
// MessageText:
//
//  The quorum log could not be created or mounted successfully.
//
export const ERROR_QUORUMLOG_OPEN_FAILED = 5028;

//
// MessageId: ERROR_CLUSTERLOG_CORRUPT
//
// MessageText:
//
//  The cluster log is corrupt.
//
export const ERROR_CLUSTERLOG_CORRUPT = 5029;

//
// MessageId: ERROR_CLUSTERLOG_RECORD_EXCEEDS_MAXSIZE
//
// MessageText:
//
//  The record could not be written to the cluster log since it exceeds the maximum size.
//
export const ERROR_CLUSTERLOG_RECORD_EXCEEDS_MAXSIZE = 5030;

//
// MessageId: ERROR_CLUSTERLOG_EXCEEDS_MAXSIZE
//
// MessageText:
//
//  The cluster log exceeds its maximum size.
//
export const ERROR_CLUSTERLOG_EXCEEDS_MAXSIZE = 5031;

//
// MessageId: ERROR_CLUSTERLOG_CHKPOINT_NOT_FOUND
//
// MessageText:
//
//  No checkpoint record was found in the cluster log.
//
export const ERROR_CLUSTERLOG_CHKPOINT_NOT_FOUND = 5032;

//
// MessageId: ERROR_CLUSTERLOG_NOT_ENOUGH_SPACE
//
// MessageText:
//
//  The minimum required disk space needed for logging is not available.
//
export const ERROR_CLUSTERLOG_NOT_ENOUGH_SPACE = 5033;

//
// MessageId: ERROR_QUORUM_OWNER_ALIVE
//
// MessageText:
//
//  The cluster node failed to take control of the quorum resource because the resource is owned by another active node.
//
export const ERROR_QUORUM_OWNER_ALIVE = 5034;

//
// MessageId: ERROR_NETWORK_NOT_AVAILABLE
//
// MessageText:
//
//  A cluster network is not available for this operation.
//
export const ERROR_NETWORK_NOT_AVAILABLE = 5035;

//
// MessageId: ERROR_NODE_NOT_AVAILABLE
//
// MessageText:
//
//  A cluster node is not available for this operation.
//
export const ERROR_NODE_NOT_AVAILABLE = 5036;

//
// MessageId: ERROR_ALL_NODES_NOT_AVAILABLE
//
// MessageText:
//
//  All cluster nodes must be running to perform this operation.
//
export const ERROR_ALL_NODES_NOT_AVAILABLE = 5037;

//
// MessageId: ERROR_RESOURCE_FAILED
//
// MessageText:
//
//  A cluster resource failed.
//
export const ERROR_RESOURCE_FAILED = 5038;

//
// MessageId: ERROR_CLUSTER_INVALID_NODE
//
// MessageText:
//
//  The cluster node is not valid.
//
export const ERROR_CLUSTER_INVALID_NODE = 5039;

//
// MessageId: ERROR_CLUSTER_NODE_EXISTS
//
// MessageText:
//
//  The cluster node already exists.
//
export const ERROR_CLUSTER_NODE_EXISTS = 5040;

//
// MessageId: ERROR_CLUSTER_JOIN_IN_PROGRESS
//
// MessageText:
//
//  A node is in the process of joining the cluster.
//
export const ERROR_CLUSTER_JOIN_IN_PROGRESS = 5041;

//
// MessageId: ERROR_CLUSTER_NODE_NOT_FOUND
//
// MessageText:
//
//  The cluster node was not found.
//
export const ERROR_CLUSTER_NODE_NOT_FOUND = 5042;

//
// MessageId: ERROR_CLUSTER_LOCAL_NODE_NOT_FOUND
//
// MessageText:
//
//  The cluster local node information was not found.
//
export const ERROR_CLUSTER_LOCAL_NODE_NOT_FOUND = 5043;

//
// MessageId: ERROR_CLUSTER_NETWORK_EXISTS
//
// MessageText:
//
//  The cluster network already exists.
//
export const ERROR_CLUSTER_NETWORK_EXISTS = 5044;

//
// MessageId: ERROR_CLUSTER_NETWORK_NOT_FOUND
//
// MessageText:
//
//  The cluster network was not found.
//
export const ERROR_CLUSTER_NETWORK_NOT_FOUND = 5045;

//
// MessageId: ERROR_CLUSTER_NETINTERFACE_EXISTS
//
// MessageText:
//
//  The cluster network interface already exists.
//
export const ERROR_CLUSTER_NETINTERFACE_EXISTS = 5046;

//
// MessageId: ERROR_CLUSTER_NETINTERFACE_NOT_FOUND
//
// MessageText:
//
//  The cluster network interface was not found.
//
export const ERROR_CLUSTER_NETINTERFACE_NOT_FOUND = 5047;

//
// MessageId: ERROR_CLUSTER_INVALID_REQUEST
//
// MessageText:
//
//  The cluster request is not valid for this object.
//
export const ERROR_CLUSTER_INVALID_REQUEST = 5048;

//
// MessageId: ERROR_CLUSTER_INVALID_NETWORK_PROVIDER
//
// MessageText:
//
//  The cluster network provider is not valid.
//
export const ERROR_CLUSTER_INVALID_NETWORK_PROVIDER = 5049;

//
// MessageId: ERROR_CLUSTER_NODE_DOWN
//
// MessageText:
//
//  The cluster node is down.
//
export const ERROR_CLUSTER_NODE_DOWN = 5050;

//
// MessageId: ERROR_CLUSTER_NODE_UNREACHABLE
//
// MessageText:
//
//  The cluster node is not reachable.
//
export const ERROR_CLUSTER_NODE_UNREACHABLE = 5051;

//
// MessageId: ERROR_CLUSTER_NODE_NOT_MEMBER
//
// MessageText:
//
//  The cluster node is not a member of the cluster.
//
export const ERROR_CLUSTER_NODE_NOT_MEMBER = 5052;

//
// MessageId: ERROR_CLUSTER_JOIN_NOT_IN_PROGRESS
//
// MessageText:
//
//  A cluster join operation is not in progress.
//
export const ERROR_CLUSTER_JOIN_NOT_IN_PROGRESS = 5053;

//
// MessageId: ERROR_CLUSTER_INVALID_NETWORK
//
// MessageText:
//
//  The cluster network is not valid.
//
export const ERROR_CLUSTER_INVALID_NETWORK = 5054;

//
// MessageId: ERROR_CLUSTER_NODE_UP
//
// MessageText:
//
//  The cluster node is up.
//
export const ERROR_CLUSTER_NODE_UP = 5056;

//
// MessageId: ERROR_CLUSTER_IPADDR_IN_USE
//
// MessageText:
//
//  The cluster IP address is already in use.
//
export const ERROR_CLUSTER_IPADDR_IN_USE = 5057;

//
// MessageId: ERROR_CLUSTER_NODE_NOT_PAUSED
//
// MessageText:
//
//  The cluster node is not paused.
//
export const ERROR_CLUSTER_NODE_NOT_PAUSED = 5058;

//
// MessageId: ERROR_CLUSTER_NO_SECURITY_CONTEXT
//
// MessageText:
//
//  No cluster security context is available.
//
export const ERROR_CLUSTER_NO_SECURITY_CONTEXT = 5059;

//
// MessageId: ERROR_CLUSTER_NETWORK_NOT_INTERNAL
//
// MessageText:
//
//  The cluster network is not configured for internal cluster communication.
//
export const ERROR_CLUSTER_NETWORK_NOT_INTERNAL = 5060;

//
// MessageId: ERROR_CLUSTER_NODE_ALREADY_UP
//
// MessageText:
//
//  The cluster node is already up.
//
export const ERROR_CLUSTER_NODE_ALREADY_UP = 5061;

//
// MessageId: ERROR_CLUSTER_NODE_ALREADY_DOWN
//
// MessageText:
//
//  The cluster node is already down.
//
export const ERROR_CLUSTER_NODE_ALREADY_DOWN = 5062;

//
// MessageId: ERROR_CLUSTER_NETWORK_ALREADY_ONLINE
//
// MessageText:
//
//  The cluster network is already online.
//
export const ERROR_CLUSTER_NETWORK_ALREADY_ONLINE = 5063;

//
// MessageId: ERROR_CLUSTER_NETWORK_ALREADY_OFFLINE
//
// MessageText:
//
//  The cluster network is already offline.
//
export const ERROR_CLUSTER_NETWORK_ALREADY_OFFLINE = 5064;

//
// MessageId: ERROR_CLUSTER_NODE_ALREADY_MEMBER
//
// MessageText:
//
//  The cluster node is already a member of the cluster.
//
export const ERROR_CLUSTER_NODE_ALREADY_MEMBER = 5065;

//
// MessageId: ERROR_CLUSTER_LAST_INTERNAL_NETWORK
//
// MessageText:
//
//  The cluster network is the only one configured for internal cluster communication between two or more active cluster nodes. The internal communication capability cannot be removed from the network.
//
export const ERROR_CLUSTER_LAST_INTERNAL_NETWORK = 5066;

//
// MessageId: ERROR_CLUSTER_NETWORK_HAS_DEPENDENTS
//
// MessageText:
//
//  One or more cluster resources depend on the network to provide service to clients. The client access capability cannot be removed from the network.
//
export const ERROR_CLUSTER_NETWORK_HAS_DEPENDENTS = 5067;

//
// MessageId: ERROR_INVALID_OPERATION_ON_QUORUM
//
// MessageText:
//
//  This operation cannot be performed on the cluster resource as it the quorum resource. You may not bring the quorum resource offline or modify its possible owners list.
//
export const ERROR_INVALID_OPERATION_ON_QUORUM = 5068;

//
// MessageId: ERROR_DEPENDENCY_NOT_ALLOWED
//
// MessageText:
//
//  The cluster quorum resource is not allowed to have any dependencies.
//
export const ERROR_DEPENDENCY_NOT_ALLOWED = 5069;

//
// MessageId: ERROR_CLUSTER_NODE_PAUSED
//
// MessageText:
//
//  The cluster node is paused.
//
export const ERROR_CLUSTER_NODE_PAUSED = 5070;

//
// MessageId: ERROR_NODE_CANT_HOST_RESOURCE
//
// MessageText:
//
//  The cluster resource cannot be brought online. The owner node cannot run this resource.
//
export const ERROR_NODE_CANT_HOST_RESOURCE = 5071;

//
// MessageId: ERROR_CLUSTER_NODE_NOT_READY
//
// MessageText:
//
//  The cluster node is not ready to perform the requested operation.
//
export const ERROR_CLUSTER_NODE_NOT_READY = 5072;

//
// MessageId: ERROR_CLUSTER_NODE_SHUTTING_DOWN
//
// MessageText:
//
//  The cluster node is shutting down.
//
export const ERROR_CLUSTER_NODE_SHUTTING_DOWN = 5073;

//
// MessageId: ERROR_CLUSTER_JOIN_ABORTED
//
// MessageText:
//
//  The cluster join operation was aborted.
//
export const ERROR_CLUSTER_JOIN_ABORTED = 5074;

//
// MessageId: ERROR_CLUSTER_INCOMPATIBLE_VERSIONS
//
// MessageText:
//
//  The cluster join operation failed due to incompatible software versions between the joining node and its sponsor.
//
export const ERROR_CLUSTER_INCOMPATIBLE_VERSIONS = 5075;

//
// MessageId: ERROR_CLUSTER_MAXNUM_OF_RESOURCES_EXCEEDED
//
// MessageText:
//
//  This resource cannot be created because the cluster has reached the limit on the number of resources it can monitor.
//
export const ERROR_CLUSTER_MAXNUM_OF_RESOURCES_EXCEEDED = 5076;

//
// MessageId: ERROR_CLUSTER_SYSTEM_CONFIG_CHANGED
//
// MessageText:
//
//  The system configuration changed during the cluster join or form operation. The join or form operation was aborted.
//
export const ERROR_CLUSTER_SYSTEM_CONFIG_CHANGED = 5077;

//
// MessageId: ERROR_CLUSTER_RESOURCE_TYPE_NOT_FOUND
//
// MessageText:
//
//  The specified resource type was not found.
//
export const ERROR_CLUSTER_RESOURCE_TYPE_NOT_FOUND = 5078;

//
// MessageId: ERROR_CLUSTER_RESTYPE_NOT_SUPPORTED
//
// MessageText:
//
//  The specified node does not support a resource of this type.  This may be due to version inconsistencies or due to the absence of the resource DLL on this node.
//
export const ERROR_CLUSTER_RESTYPE_NOT_SUPPORTED = 5079;

//
// MessageId: ERROR_CLUSTER_RESNAME_NOT_FOUND
//
// MessageText:
//
//  The specified resource name is not supported by this resource DLL. This may be due to a bad (or changed) name supplied to the resource DLL.
//
export const ERROR_CLUSTER_RESNAME_NOT_FOUND = 5080;

//
// MessageId: ERROR_CLUSTER_NO_RPC_PACKAGES_REGISTERED
//
// MessageText:
//
//  No authentication package could be registered with the RPC server.
//
export const ERROR_CLUSTER_NO_RPC_PACKAGES_REGISTERED = 5081;

//
// MessageId: ERROR_CLUSTER_OWNER_NOT_IN_PREFLIST
//
// MessageText:
//
//  You cannot bring the group online because the owner of the group is not in the preferred list for the group. To change the owner node for the group, move the group.
//
export const ERROR_CLUSTER_OWNER_NOT_IN_PREFLIST = 5082;

//
// MessageId: ERROR_CLUSTER_DATABASE_SEQMISMATCH
//
// MessageText:
//
//  The join operation failed because the cluster database sequence number has changed or is incompatible with the locker node. This may happen during a join operation if the cluster database was changing during the join.
//
export const ERROR_CLUSTER_DATABASE_SEQMISMATCH = 5083;

//
// MessageId: ERROR_RESMON_INVALID_STATE
//
// MessageText:
//
//  The resource monitor will not allow the fail operation to be performed while the resource is in its current state. This may happen if the resource is in a pending state.
//
export const ERROR_RESMON_INVALID_STATE = 5084;

//
// MessageId: ERROR_CLUSTER_GUM_NOT_LOCKER
//
// MessageText:
//
//  A non locker code got a request to reserve the lock for making global updates.
//
export const ERROR_CLUSTER_GUM_NOT_LOCKER = 5085;

//
// MessageId: ERROR_QUORUM_DISK_NOT_FOUND
//
// MessageText:
//
//  The quorum disk could not be located by the cluster service.
//
export const ERROR_QUORUM_DISK_NOT_FOUND = 5086;

//
// MessageId: ERROR_DATABASE_BACKUP_CORRUPT
//
// MessageText:
//
//  The backed up cluster database is possibly corrupt.
//
export const ERROR_DATABASE_BACKUP_CORRUPT = 5087;

//
// MessageId: ERROR_CLUSTER_NODE_ALREADY_HAS_DFS_ROOT
//
// MessageText:
//
//  A DFS root already exists in this cluster node.
//
export const ERROR_CLUSTER_NODE_ALREADY_HAS_DFS_ROOT = 5088;

//
// MessageId: ERROR_RESOURCE_PROPERTY_UNCHANGEABLE
//
// MessageText:
//
//  An attempt to modify a resource property failed because it conflicts with another existing property.
//
export const ERROR_RESOURCE_PROPERTY_UNCHANGEABLE = 5089;

/*
 Codes from 4300 through 5889 overlap with codes in ds\published\inc\apperr2.w.
 Do not add any more error codes in that range.
*/
//
// MessageId: ERROR_CLUSTER_MEMBERSHIP_INVALID_STATE
//
// MessageText:
//
//  An operation was attempted that is incompatible with the current membership state of the node.
//
export const ERROR_CLUSTER_MEMBERSHIP_INVALID_STATE = 5890;

//
// MessageId: ERROR_CLUSTER_QUORUMLOG_NOT_FOUND
//
// MessageText:
//
//  The quorum resource does not contain the quorum log.
//
export const ERROR_CLUSTER_QUORUMLOG_NOT_FOUND = 5891;

//
// MessageId: ERROR_CLUSTER_MEMBERSHIP_HALT
//
// MessageText:
//
//  The membership engine requested shutdown of the cluster service on this node.
//
export const ERROR_CLUSTER_MEMBERSHIP_HALT = 5892;

//
// MessageId: ERROR_CLUSTER_INSTANCE_ID_MISMATCH
//
// MessageText:
//
//  The join operation failed because the cluster instance ID of the joining node does not match the cluster instance ID of the sponsor node.
//
export const ERROR_CLUSTER_INSTANCE_ID_MISMATCH = 5893;

//
// MessageId: ERROR_CLUSTER_NETWORK_NOT_FOUND_FOR_IP
//
// MessageText:
//
//  A matching network for the specified IP address could not be found. Please also specify a subnet mask and a cluster network.
//
export const ERROR_CLUSTER_NETWORK_NOT_FOUND_FOR_IP = 5894;

//
// MessageId: ERROR_CLUSTER_PROPERTY_DATA_TYPE_MISMATCH
//
// MessageText:
//
//  The actual data type of the property did not match the expected data type of the property.
//
export const ERROR_CLUSTER_PROPERTY_DATA_TYPE_MISMATCH = 5895;

//
// MessageId: ERROR_CLUSTER_EVICT_WITHOUT_CLEANUP
//
// MessageText:
//
//  The cluster node was evicted from the cluster successfully, but the node was not cleaned up.  Extended status information explaining why the node was not cleaned up is available.
//
export const ERROR_CLUSTER_EVICT_WITHOUT_CLEANUP = 5896;

//
// MessageId: ERROR_CLUSTER_PARAMETER_MISMATCH
//
// MessageText:
//
//  Two or more parameter values specified for a resource's properties are in conflict.
//
export const ERROR_CLUSTER_PARAMETER_MISMATCH = 5897;

//
// MessageId: ERROR_NODE_CANNOT_BE_CLUSTERED
//
// MessageText:
//
//  This computer cannot be made a member of a cluster.
//
export const ERROR_NODE_CANNOT_BE_CLUSTERED = 5898;

//
// MessageId: ERROR_CLUSTER_WRONG_OS_VERSION
//
// MessageText:
//
//  This computer cannot be made a member of a cluster because it does not have the correct version of Windows installed.
//
export const ERROR_CLUSTER_WRONG_OS_VERSION = 5899;

//
// MessageId: ERROR_CLUSTER_CANT_CREATE_DUP_CLUSTER_NAME
//
// MessageText:
//
//  A cluster cannot be created with the specified cluster name because that cluster name is already in use. Specify a different name for the cluster.
//
export const ERROR_CLUSTER_CANT_CREATE_DUP_CLUSTER_NAME = 5900;

//
// MessageId: ERROR_CLUSCFG_ALREADY_COMMITTED
//
// MessageText:
//
//  The cluster configuration action has already been committed.
//
export const ERROR_CLUSCFG_ALREADY_COMMITTED = 5901;

//
// MessageId: ERROR_CLUSCFG_ROLLBACK_FAILED
//
// MessageText:
//
//  The cluster configuration action could not be rolled back.
//
export const ERROR_CLUSCFG_ROLLBACK_FAILED = 5902;

//
// MessageId: ERROR_CLUSCFG_SYSTEM_DISK_DRIVE_LETTER_CONFLICT
//
// MessageText:
//
//  The drive letter assigned to a system disk on one node conflicted with the drive letter assigned to a disk on another node.
//
export const ERROR_CLUSCFG_SYSTEM_DISK_DRIVE_LETTER_CONFLICT = 5903;

//
// MessageId: ERROR_CLUSTER_OLD_VERSION
//
// MessageText:
//
//  One or more nodes in the cluster are running a version of Windows that does not support this operation.
//
export const ERROR_CLUSTER_OLD_VERSION = 5904;

//
// MessageId: ERROR_CLUSTER_MISMATCHED_COMPUTER_ACCT_NAME
//
// MessageText:
//
//  The name of the corresponding computer account doesn't match the Network Name for this resource.
//
export const ERROR_CLUSTER_MISMATCHED_COMPUTER_ACCT_NAME = 5905;

////////////////////////////////////
//                                //
//     EFS Error Codes            //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_ENCRYPTION_FAILED
//
// MessageText:
//
//  The specified file could not be encrypted.
//
export const ERROR_ENCRYPTION_FAILED = 6000;

//
// MessageId: ERROR_DECRYPTION_FAILED
//
// MessageText:
//
//  The specified file could not be decrypted.
//
export const ERROR_DECRYPTION_FAILED = 6001;

//
// MessageId: ERROR_FILE_ENCRYPTED
//
// MessageText:
//
//  The specified file is encrypted and the user does not have the ability to decrypt it.
//
export const ERROR_FILE_ENCRYPTED = 6002;

//
// MessageId: ERROR_NO_RECOVERY_POLICY
//
// MessageText:
//
//  There is no valid encryption recovery policy configured for this system.
//
export const ERROR_NO_RECOVERY_POLICY = 6003;

//
// MessageId: ERROR_NO_EFS
//
// MessageText:
//
//  The required encryption driver is not loaded for this system.
//
export const ERROR_NO_EFS = 6004;

//
// MessageId: ERROR_WRONG_EFS
//
// MessageText:
//
//  The file was encrypted with a different encryption driver than is currently loaded.
//
export const ERROR_WRONG_EFS = 6005;

//
// MessageId: ERROR_NO_USER_KEYS
//
// MessageText:
//
//  There are no EFS keys defined for the user.
//
export const ERROR_NO_USER_KEYS = 6006;

//
// MessageId: ERROR_FILE_NOT_ENCRYPTED
//
// MessageText:
//
//  The specified file is not encrypted.
//
export const ERROR_FILE_NOT_ENCRYPTED = 6007;

//
// MessageId: ERROR_NOT_EXPORT_FORMAT
//
// MessageText:
//
//  The specified file is not in the defined EFS export format.
//
export const ERROR_NOT_EXPORT_FORMAT = 6008;

//
// MessageId: ERROR_FILE_READ_ONLY
//
// MessageText:
//
//  The specified file is read only.
//
export const ERROR_FILE_READ_ONLY = 6009;

//
// MessageId: ERROR_DIR_EFS_DISALLOWED
//
// MessageText:
//
//  The directory has been disabled for encryption.
//
export const ERROR_DIR_EFS_DISALLOWED = 6010;

//
// MessageId: ERROR_EFS_SERVER_NOT_TRUSTED
//
// MessageText:
//
//  The server is not trusted for remote encryption operation.
//
export const ERROR_EFS_SERVER_NOT_TRUSTED = 6011;

//
// MessageId: ERROR_BAD_RECOVERY_POLICY
//
// MessageText:
//
//  Recovery policy configured for this system contains invalid recovery certificate.
//
export const ERROR_BAD_RECOVERY_POLICY = 6012;

//
// MessageId: ERROR_EFS_ALG_BLOB_TOO_BIG
//
// MessageText:
//
//  The encryption algorithm used on the source file needs a bigger key buffer than the one on the destination file.
//
export const ERROR_EFS_ALG_BLOB_TOO_BIG = 6013;

//
// MessageId: ERROR_VOLUME_NOT_SUPPORT_EFS
//
// MessageText:
//
//  The disk partition does not support file encryption.
//
export const ERROR_VOLUME_NOT_SUPPORT_EFS = 6014;

//
// MessageId: ERROR_EFS_DISABLED
//
// MessageText:
//
//  This machine is disabled for file encryption.
//
export const ERROR_EFS_DISABLED = 6015;

//
// MessageId: ERROR_EFS_VERSION_NOT_SUPPORT
//
// MessageText:
//
//  A newer system is required to decrypt this encrypted file.
//
export const ERROR_EFS_VERSION_NOT_SUPPORT = 6016;

// This message number is for historical purposes and cannot be changed or re-used.
//
// MessageId: ERROR_NO_BROWSER_SERVERS_FOUND
//
// MessageText:
//
//  The list of servers for this workgroup is not currently available
//
export const ERROR_NO_BROWSER_SERVERS_FOUND = 6118;

//////////////////////////////////////////////////////////////////
//                                                              //
// Task Scheduler Error Codes that NET START must understand    //
//                                                              //
//////////////////////////////////////////////////////////////////
//
// MessageId: SCHED_E_SERVICE_NOT_LOCALSYSTEM
//
// MessageText:
//
//  The Task Scheduler service must be configured to run in the System account to function properly.  Individual tasks may be configured to run in other accounts.
//
export const SCHED_E_SERVICE_NOT_LOCALSYSTEM = 6200;

////////////////////////////////////
//                                //
// Terminal Server Error Codes    //
//                                //
////////////////////////////////////
//
// MessageId: ERROR_CTX_WINSTATION_NAME_INVALID
//
// MessageText:
//
//  The specified session name is invalid.
//
export const ERROR_CTX_WINSTATION_NAME_INVALID = 7001;

//
// MessageId: ERROR_CTX_INVALID_PD
//
// MessageText:
//
//  The specified protocol driver is invalid.
//
export const ERROR_CTX_INVALID_PD = 7002;

//
// MessageId: ERROR_CTX_PD_NOT_FOUND
//
// MessageText:
//
//  The specified protocol driver was not found in the system path.
//
export const ERROR_CTX_PD_NOT_FOUND = 7003;

//
// MessageId: ERROR_CTX_WD_NOT_FOUND
//
// MessageText:
//
//  The specified terminal connection driver was not found in the system path.
//
export const ERROR_CTX_WD_NOT_FOUND = 7004;

//
// MessageId: ERROR_CTX_CANNOT_MAKE_EVENTLOG_ENTRY
//
// MessageText:
//
//  A registry key for event logging could not be created for this session.
//
export const ERROR_CTX_CANNOT_MAKE_EVENTLOG_ENTRY = 7005;

//
// MessageId: ERROR_CTX_SERVICE_NAME_COLLISION
//
// MessageText:
//
//  A service with the same name already exists on the system.
//
export const ERROR_CTX_SERVICE_NAME_COLLISION = 7006;

//
// MessageId: ERROR_CTX_CLOSE_PENDING
//
// MessageText:
//
//  A close operation is pending on the session.
//
export const ERROR_CTX_CLOSE_PENDING = 7007;

//
// MessageId: ERROR_CTX_NO_OUTBUF
//
// MessageText:
//
//  There are no free output buffers available.
//
export const ERROR_CTX_NO_OUTBUF = 7008;

//
// MessageId: ERROR_CTX_MODEM_INF_NOT_FOUND
//
// MessageText:
//
//  The MODEM.INF file was not found.
//
export const ERROR_CTX_MODEM_INF_NOT_FOUND = 7009;

//
// MessageId: ERROR_CTX_INVALID_MODEMNAME
//
// MessageText:
//
//  The modem name was not found in MODEM.INF.
//
export const ERROR_CTX_INVALID_MODEMNAME = 7010;

//
// MessageId: ERROR_CTX_MODEM_RESPONSE_ERROR
//
// MessageText:
//
//  The modem did not accept the command sent to it. Verify that the configured modem name matches the attached modem.
//
export const ERROR_CTX_MODEM_RESPONSE_ERROR = 7011;

//
// MessageId: ERROR_CTX_MODEM_RESPONSE_TIMEOUT
//
// MessageText:
//
//  The modem did not respond to the command sent to it. Verify that the modem is properly cabled and powered on.
//
export const ERROR_CTX_MODEM_RESPONSE_TIMEOUT = 7012;

//
// MessageId: ERROR_CTX_MODEM_RESPONSE_NO_CARRIER
//
// MessageText:
//
//  Carrier detect has failed or carrier has been dropped due to disconnect.
//
export const ERROR_CTX_MODEM_RESPONSE_NO_CARRIER = 7013;

//
// MessageId: ERROR_CTX_MODEM_RESPONSE_NO_DIALTONE
//
// MessageText:
//
//  Dial tone not detected within the required time. Verify that the phone cable is properly attached and functional.
//
export const ERROR_CTX_MODEM_RESPONSE_NO_DIALTONE = 7014;

//
// MessageId: ERROR_CTX_MODEM_RESPONSE_BUSY
//
// MessageText:
//
//  Busy signal detected at remote site on callback.
//
export const ERROR_CTX_MODEM_RESPONSE_BUSY = 7015;

//
// MessageId: ERROR_CTX_MODEM_RESPONSE_VOICE
//
// MessageText:
//
//  Voice detected at remote site on callback.
//
export const ERROR_CTX_MODEM_RESPONSE_VOICE = 7016;

//
// MessageId: ERROR_CTX_TD_ERROR
//
// MessageText:
//
//  Transport driver error
//
export const ERROR_CTX_TD_ERROR = 7017;

//
// MessageId: ERROR_CTX_WINSTATION_NOT_FOUND
//
// MessageText:
//
//  The specified session cannot be found.
//
export const ERROR_CTX_WINSTATION_NOT_FOUND = 7022;

//
// MessageId: ERROR_CTX_WINSTATION_ALREADY_EXISTS
//
// MessageText:
//
//  The specified session name is already in use.
//
export const ERROR_CTX_WINSTATION_ALREADY_EXISTS = 7023;

//
// MessageId: ERROR_CTX_WINSTATION_BUSY
//
// MessageText:
//
//  The requested operation cannot be completed because the terminal connection is currently busy processing a connect, disconnect, reset, or delete operation.
//
export const ERROR_CTX_WINSTATION_BUSY = 7024;

//
// MessageId: ERROR_CTX_BAD_VIDEO_MODE
//
// MessageText:
//
//  An attempt has been made to connect to a session whose video mode is not supported by the current client.
//
export const ERROR_CTX_BAD_VIDEO_MODE = 7025;

//
// MessageId: ERROR_CTX_GRAPHICS_INVALID
//
// MessageText:
//
//  The application attempted to enable DOS graphics mode.
//  DOS graphics mode is not supported.
//
export const ERROR_CTX_GRAPHICS_INVALID = 7035;

//
// MessageId: ERROR_CTX_LOGON_DISABLED
//
// MessageText:
//
//  Your interactive logon privilege has been disabled.
//  Please contact your administrator.
//
export const ERROR_CTX_LOGON_DISABLED = 7037;

//
// MessageId: ERROR_CTX_NOT_CONSOLE
//
// MessageText:
//
//  The requested operation can be performed only on the system console.
//  This is most often the result of a driver or system DLL requiring direct console access.
//
export const ERROR_CTX_NOT_CONSOLE = 7038;

//
// MessageId: ERROR_CTX_CLIENT_QUERY_TIMEOUT
//
// MessageText:
//
//  The client failed to respond to the server connect message.
//
export const ERROR_CTX_CLIENT_QUERY_TIMEOUT = 7040;

//
// MessageId: ERROR_CTX_CONSOLE_DISCONNECT
//
// MessageText:
//
//  Disconnecting the console session is not supported.
//
export const ERROR_CTX_CONSOLE_DISCONNECT = 7041;

//
// MessageId: ERROR_CTX_CONSOLE_CONNECT
//
// MessageText:
//
//  Reconnecting a disconnected session to the console is not supported.
//
export const ERROR_CTX_CONSOLE_CONNECT = 7042;

//
// MessageId: ERROR_CTX_SHADOW_DENIED
//
// MessageText:
//
//  The request to control another session remotely was denied.
//
export const ERROR_CTX_SHADOW_DENIED = 7044;

//
// MessageId: ERROR_CTX_WINSTATION_ACCESS_DENIED
//
// MessageText:
//
//  The requested session access is denied.
//
export const ERROR_CTX_WINSTATION_ACCESS_DENIED = 7045;

//
// MessageId: ERROR_CTX_INVALID_WD
//
// MessageText:
//
//  The specified terminal connection driver is invalid.
//
export const ERROR_CTX_INVALID_WD = 7049;

//
// MessageId: ERROR_CTX_SHADOW_INVALID
//
// MessageText:
//
//  The requested session cannot be controlled remotely.
//  This may be because the session is disconnected or does not currently have a user logged on.
//
export const ERROR_CTX_SHADOW_INVALID = 7050;

//
// MessageId: ERROR_CTX_SHADOW_DISABLED
//
// MessageText:
//
//  The requested session is not configured to allow remote control.
//
export const ERROR_CTX_SHADOW_DISABLED = 7051;

//
// MessageId: ERROR_CTX_CLIENT_LICENSE_IN_USE
//
// MessageText:
//
//  Your request to connect to this Terminal Server has been rejected. Your Terminal Server client license number is currently being used by another user.
//  Please call your system administrator to obtain a unique license number.
//
export const ERROR_CTX_CLIENT_LICENSE_IN_USE = 7052;

//
// MessageId: ERROR_CTX_CLIENT_LICENSE_NOT_SET
//
// MessageText:
//
//  Your request to connect to this Terminal Server has been rejected. Your Terminal Server client license number has not been entered for this copy of the Terminal Server client.
//  Please contact your system administrator.
//
export const ERROR_CTX_CLIENT_LICENSE_NOT_SET = 7053;

//
// MessageId: ERROR_CTX_LICENSE_NOT_AVAILABLE
//
// MessageText:
//
//  The system has reached its licensed logon limit.
//  Please try again later.
//
export const ERROR_CTX_LICENSE_NOT_AVAILABLE = 7054;

//
// MessageId: ERROR_CTX_LICENSE_CLIENT_INVALID
//
// MessageText:
//
//  The client you are using is not licensed to use this system.  Your logon request is denied.
//
export const ERROR_CTX_LICENSE_CLIENT_INVALID = 7055;

//
// MessageId: ERROR_CTX_LICENSE_EXPIRED
//
// MessageText:
//
//  The system license has expired.  Your logon request is denied.
//
export const ERROR_CTX_LICENSE_EXPIRED = 7056;

//
// MessageId: ERROR_CTX_SHADOW_NOT_RUNNING
//
// MessageText:
//
//  Remote control could not be terminated because the specified session is not currently being remotely controlled.
//
export const ERROR_CTX_SHADOW_NOT_RUNNING = 7057;

//
// MessageId: ERROR_CTX_SHADOW_ENDED_BY_MODE_CHANGE
//
// MessageText:
//
//  The remote control of the console was terminated because the display mode was changed. Changing the display mode in a remote control session is not supported.
//
export const ERROR_CTX_SHADOW_ENDED_BY_MODE_CHANGE = 7058;

//
// MessageId: ERROR_ACTIVATION_COUNT_EXCEEDED
//
// MessageText:
//
//  Activation has already been reset the maximum number of times for this installation. Your activation timer will not be cleared.
//
export const ERROR_ACTIVATION_COUNT_EXCEEDED = 7059;

///////////////////////////////////////////////////
//                                                /
//             Traffic Control Error Codes        /
//                                                /
//                  7500 to  7999                 /
//                                                /
//         defined in: tcerror.h                  /
///////////////////////////////////////////////////
///////////////////////////////////////////////////
//                                                /
//             Active Directory Error Codes       /
//                                                /
//                  8000 to  8999                 /
///////////////////////////////////////////////////
// *****************
// FACILITY_FILE_REPLICATION_SERVICE
// *****************
//
// MessageId: FRS_ERR_INVALID_API_SEQUENCE
//
// MessageText:
//
//  The file replication service API was called incorrectly.
//
export const FRS_ERR_INVALID_API_SEQUENCE = 8001;

//
// MessageId: FRS_ERR_STARTING_SERVICE
//
// MessageText:
//
//  The file replication service cannot be started.
//
export const FRS_ERR_STARTING_SERVICE = 8002;

//
// MessageId: FRS_ERR_STOPPING_SERVICE
//
// MessageText:
//
//  The file replication service cannot be stopped.
//
export const FRS_ERR_STOPPING_SERVICE = 8003;

//
// MessageId: FRS_ERR_INTERNAL_API
//
// MessageText:
//
//  The file replication service API terminated the request.
//  The event log may have more information.
//
export const FRS_ERR_INTERNAL_API = 8004;

//
// MessageId: FRS_ERR_INTERNAL
//
// MessageText:
//
//  The file replication service terminated the request.
//  The event log may have more information.
//
export const FRS_ERR_INTERNAL = 8005;

//
// MessageId: FRS_ERR_SERVICE_COMM
//
// MessageText:
//
//  The file replication service cannot be contacted.
//  The event log may have more information.
//
export const FRS_ERR_SERVICE_COMM = 8006;

//
// MessageId: FRS_ERR_INSUFFICIENT_PRIV
//
// MessageText:
//
//  The file replication service cannot satisfy the request because the user has insufficient privileges.
//  The event log may have more information.
//
export const FRS_ERR_INSUFFICIENT_PRIV = 8007;

//
// MessageId: FRS_ERR_AUTHENTICATION
//
// MessageText:
//
//  The file replication service cannot satisfy the request because authenticated RPC is not available.
//  The event log may have more information.
//
export const FRS_ERR_AUTHENTICATION = 8008;

//
// MessageId: FRS_ERR_PARENT_INSUFFICIENT_PRIV
//
// MessageText:
//
//  The file replication service cannot satisfy the request because the user has insufficient privileges on the domain controller.
//  The event log may have more information.
//
export const FRS_ERR_PARENT_INSUFFICIENT_PRIV = 8009;

//
// MessageId: FRS_ERR_PARENT_AUTHENTICATION
//
// MessageText:
//
//  The file replication service cannot satisfy the request because authenticated RPC is not available on the domain controller.
//  The event log may have more information.
//
export const FRS_ERR_PARENT_AUTHENTICATION = 8010;

//
// MessageId: FRS_ERR_CHILD_TO_PARENT_COMM
//
// MessageText:
//
//  The file replication service cannot communicate with the file replication service on the domain controller.
//  The event log may have more information.
//
export const FRS_ERR_CHILD_TO_PARENT_COMM = 8011;

//
// MessageId: FRS_ERR_PARENT_TO_CHILD_COMM
//
// MessageText:
//
//  The file replication service on the domain controller cannot communicate with the file replication service on this computer.
//  The event log may have more information.
//
export const FRS_ERR_PARENT_TO_CHILD_COMM = 8012;

//
// MessageId: FRS_ERR_SYSVOL_POPULATE
//
// MessageText:
//
//  The file replication service cannot populate the system volume because of an internal error.
//  The event log may have more information.
//
export const FRS_ERR_SYSVOL_POPULATE = 8013;

//
// MessageId: FRS_ERR_SYSVOL_POPULATE_TIMEOUT
//
// MessageText:
//
//  The file replication service cannot populate the system volume because of an internal timeout.
//  The event log may have more information.
//
export const FRS_ERR_SYSVOL_POPULATE_TIMEOUT = 8014;

//
// MessageId: FRS_ERR_SYSVOL_IS_BUSY
//
// MessageText:
//
//  The file replication service cannot process the request. The system volume is busy with a previous request.
//
export const FRS_ERR_SYSVOL_IS_BUSY = 8015;

//
// MessageId: FRS_ERR_SYSVOL_DEMOTE
//
// MessageText:
//
//  The file replication service cannot stop replicating the system volume because of an internal error.
//  The event log may have more information.
//
export const FRS_ERR_SYSVOL_DEMOTE = 8016;

//
// MessageId: FRS_ERR_INVALID_SERVICE_PARAMETER
//
// MessageText:
//
//  The file replication service detected an invalid parameter.
//
export const FRS_ERR_INVALID_SERVICE_PARAMETER = 8017;

// *****************
// FACILITY DIRECTORY SERVICE
// *****************
//
// MessageId: ERROR_DS_NOT_INSTALLED
//
// MessageText:
//
//  An error occurred while installing the directory service. For more information, see the event log.
//
export const ERROR_DS_NOT_INSTALLED = 8200;

//
// MessageId: ERROR_DS_MEMBERSHIP_EVALUATED_LOCALLY
//
// MessageText:
//
//  The directory service evaluated group memberships locally.
//
export const ERROR_DS_MEMBERSHIP_EVALUATED_LOCALLY = 8201;

//
// MessageId: ERROR_DS_NO_ATTRIBUTE_OR_VALUE
//
// MessageText:
//
//  The specified directory service attribute or value does not exist.
//
export const ERROR_DS_NO_ATTRIBUTE_OR_VALUE = 8202;

//
// MessageId: ERROR_DS_INVALID_ATTRIBUTE_SYNTAX
//
// MessageText:
//
//  The attribute syntax specified to the directory service is invalid.
//
export const ERROR_DS_INVALID_ATTRIBUTE_SYNTAX = 8203;

//
// MessageId: ERROR_DS_ATTRIBUTE_TYPE_UNDEFINED
//
// MessageText:
//
//  The attribute type specified to the directory service is not defined.
//
export const ERROR_DS_ATTRIBUTE_TYPE_UNDEFINED = 8204;

//
// MessageId: ERROR_DS_ATTRIBUTE_OR_VALUE_EXISTS
//
// MessageText:
//
//  The specified directory service attribute or value already exists.
//
export const ERROR_DS_ATTRIBUTE_OR_VALUE_EXISTS = 8205;

//
// MessageId: ERROR_DS_BUSY
//
// MessageText:
//
//  The directory service is busy.
//
export const ERROR_DS_BUSY = 8206;

//
// MessageId: ERROR_DS_UNAVAILABLE
//
// MessageText:
//
//  The directory service is unavailable.
//
export const ERROR_DS_UNAVAILABLE = 8207;

//
// MessageId: ERROR_DS_NO_RIDS_ALLOCATED
//
// MessageText:
//
//  The directory service was unable to allocate a relative identifier.
//
export const ERROR_DS_NO_RIDS_ALLOCATED = 8208;

//
// MessageId: ERROR_DS_NO_MORE_RIDS
//
// MessageText:
//
//  The directory service has exhausted the pool of relative identifiers.
//
export const ERROR_DS_NO_MORE_RIDS = 8209;

//
// MessageId: ERROR_DS_INCORRECT_ROLE_OWNER
//
// MessageText:
//
//  The requested operation could not be performed because the directory service is not the master for that type of operation.
//
export const ERROR_DS_INCORRECT_ROLE_OWNER = 8210;

//
// MessageId: ERROR_DS_RIDMGR_INIT_ERROR
//
// MessageText:
//
//  The directory service was unable to initialize the subsystem that allocates relative identifiers.
//
export const ERROR_DS_RIDMGR_INIT_ERROR = 8211;

//
// MessageId: ERROR_DS_OBJ_CLASS_VIOLATION
//
// MessageText:
//
//  The requested operation did not satisfy one or more export constraints associated with the class of the object.
//
export const ERROR_DS_OBJ_CLASS_VIOLATION = 8212;

//
// MessageId: ERROR_DS_CANT_ON_NON_LEAF
//
// MessageText:
//
//  The directory service can perform the requested operation only on a leaf object.
//
export const ERROR_DS_CANT_ON_NON_LEAF = 8213;

//
// MessageId: ERROR_DS_CANT_ON_RDN
//
// MessageText:
//
//  The directory service cannot perform the requested operation on the RDN attribute of an object.
//
export const ERROR_DS_CANT_ON_RDN = 8214;

//
// MessageId: ERROR_DS_CANT_MOD_OBJ_CLASS
//
// MessageText:
//
//  The directory service detected an attempt to modify the object class of an object.
//
export const ERROR_DS_CANT_MOD_OBJ_CLASS = 8215;

//
// MessageId: ERROR_DS_CROSS_DOM_MOVE_ERROR
//
// MessageText:
//
//  The requested cross-domain move operation could not be performed.
//
export const ERROR_DS_CROSS_DOM_MOVE_ERROR = 8216;

//
// MessageId: ERROR_DS_GC_NOT_AVAILABLE
//
// MessageText:
//
//  Unable to contact the global catalog server.
//
export const ERROR_DS_GC_NOT_AVAILABLE = 8217;

//
// MessageId: ERROR_SHARED_POLICY
//
// MessageText:
//
//  The policy object is shared and can only be modified at the root.
//
export const ERROR_SHARED_POLICY = 8218;

//
// MessageId: ERROR_POLICY_OBJECT_NOT_FOUND
//
// MessageText:
//
//  The policy object does not exist.
//
export const ERROR_POLICY_OBJECT_NOT_FOUND = 8219;

//
// MessageId: ERROR_POLICY_ONLY_IN_DS
//
// MessageText:
//
//  The requested policy information is only in the directory service.
//
export const ERROR_POLICY_ONLY_IN_DS = 8220;

//
// MessageId: ERROR_PROMOTION_ACTIVE
//
// MessageText:
//
//  A domain controller promotion is currently active.
//
export const ERROR_PROMOTION_ACTIVE = 8221;

//
// MessageId: ERROR_NO_PROMOTION_ACTIVE
//
// MessageText:
//
//  A domain controller promotion is not currently active
//
export const ERROR_NO_PROMOTION_ACTIVE = 8222;

// 8223 unused
//
// MessageId: ERROR_DS_OPERATIONS_ERROR
//
// MessageText:
//
//  An operations error occurred.
//
export const ERROR_DS_OPERATIONS_ERROR = 8224;

//
// MessageId: ERROR_DS_PROTOCOL_ERROR
//
// MessageText:
//
//  A protocol error occurred.
//
export const ERROR_DS_PROTOCOL_ERROR = 8225;

//
// MessageId: ERROR_DS_TIMELIMIT_EXCEEDED
//
// MessageText:
//
//  The time limit for this request was exceeded.
//
export const ERROR_DS_TIMELIMIT_EXCEEDED = 8226;

//
// MessageId: ERROR_DS_SIZELIMIT_EXCEEDED
//
// MessageText:
//
//  The size limit for this request was exceeded.
//
export const ERROR_DS_SIZELIMIT_EXCEEDED = 8227;

//
// MessageId: ERROR_DS_ADMIN_LIMIT_EXCEEDED
//
// MessageText:
//
//  The administrative limit for this request was exceeded.
//
export const ERROR_DS_ADMIN_LIMIT_EXCEEDED = 8228;

//
// MessageId: ERROR_DS_COMPARE_FALSE
//
// MessageText:
//
//  The compare response was false.
//
export const ERROR_DS_COMPARE_FALSE = 8229;

//
// MessageId: ERROR_DS_COMPARE_TRUE
//
// MessageText:
//
//  The compare response was true.
//
export const ERROR_DS_COMPARE_TRUE = 8230;

//
// MessageId: ERROR_DS_AUTH_METHOD_NOT_SUPPORTED
//
// MessageText:
//
//  The requested authentication method is not supported by the server.
//
export const ERROR_DS_AUTH_METHOD_NOT_SUPPORTED = 8231;

//
// MessageId: ERROR_DS_STRONG_AUTH_REQUIRED
//
// MessageText:
//
//  A more secure authentication method is required for this server.
//
export const ERROR_DS_STRONG_AUTH_REQUIRED = 8232;

//
// MessageId: ERROR_DS_INAPPROPRIATE_AUTH
//
// MessageText:
//
//  Inappropriate authentication.
//
export const ERROR_DS_INAPPROPRIATE_AUTH = 8233;

//
// MessageId: ERROR_DS_AUTH_UNKNOWN
//
// MessageText:
//
//  The authentication mechanism is unknown.
//
export const ERROR_DS_AUTH_UNKNOWN = 8234;

//
// MessageId: ERROR_DS_REFERRAL
//
// MessageText:
//
//  A referral was returned from the server.
//
export const ERROR_DS_REFERRAL = 8235;

//
// MessageId: ERROR_DS_UNAVAILABLE_CRIT_EXTENSION
//
// MessageText:
//
//  The server does not support the requested critical extension.
//
export const ERROR_DS_UNAVAILABLE_CRIT_EXTENSION = 8236;

//
// MessageId: ERROR_DS_CONFIDENTIALITY_REQUIRED
//
// MessageText:
//
//  This request requires a secure connection.
//
export const ERROR_DS_CONFIDENTIALITY_REQUIRED = 8237;

//
// MessageId: ERROR_DS_INAPPROPRIATE_MATCHING
//
// MessageText:
//
//  Inappropriate matching.
//
export const ERROR_DS_INAPPROPRIATE_MATCHING = 8238;

//
// MessageId: ERROR_DS_NO_SUCH_OBJECT
//
// MessageText:
//
//  There is no such object on the server.
//
export const ERROR_DS_NO_SUCH_OBJECT = 8240;

//
// MessageId: ERROR_DS_ALIAS_PROBLEM
//
// MessageText:
//
//  There is an alias problem.
//
export const ERROR_DS_ALIAS_PROBLEM = 8241;

//
// MessageId: ERROR_DS_INVALID_DN_SYNTAX
//
// MessageText:
//
//  An invalid dn syntax has been specified.
//
export const ERROR_DS_INVALID_DN_SYNTAX = 8242;

//
// MessageId: ERROR_DS_IS_LEAF
//
// MessageText:
//
//  The object is a leaf object.
//
export const ERROR_DS_IS_LEAF = 8243;

//
// MessageId: ERROR_DS_ALIAS_DEREF_PROBLEM
//
// MessageText:
//
//  There is an alias dereferencing problem.
//
export const ERROR_DS_ALIAS_DEREF_PROBLEM = 8244;

//
// MessageId: ERROR_DS_UNWILLING_TO_PERFORM
//
// MessageText:
//
//  The server is unwilling to process the request.
//
export const ERROR_DS_UNWILLING_TO_PERFORM = 8245;

//
// MessageId: ERROR_DS_LOOP_DETECT
//
// MessageText:
//
//  A loop has been detected.
//
export const ERROR_DS_LOOP_DETECT = 8246;

//
// MessageId: ERROR_DS_NAMING_VIOLATION
//
// MessageText:
//
//  There is a naming violation.
//
export const ERROR_DS_NAMING_VIOLATION = 8247;

//
// MessageId: ERROR_DS_OBJECT_RESULTS_TOO_LARGE
//
// MessageText:
//
//  The result set is too large.
//
export const ERROR_DS_OBJECT_RESULTS_TOO_LARGE = 8248;

//
// MessageId: ERROR_DS_AFFECTS_MULTIPLE_DSAS
//
// MessageText:
//
//  The operation affects multiple DSAs
//
export const ERROR_DS_AFFECTS_MULTIPLE_DSAS = 8249;

//
// MessageId: ERROR_DS_SERVER_DOWN
//
// MessageText:
//
//  The server is not operational.
//
export const ERROR_DS_SERVER_DOWN = 8250;

//
// MessageId: ERROR_DS_LOCAL_ERROR
//
// MessageText:
//
//  A local error has occurred.
//
export const ERROR_DS_LOCAL_ERROR = 8251;

//
// MessageId: ERROR_DS_ENCODING_ERROR
//
// MessageText:
//
//  An encoding error has occurred.
//
export const ERROR_DS_ENCODING_ERROR = 8252;

//
// MessageId: ERROR_DS_DECODING_ERROR
//
// MessageText:
//
//  A decoding error has occurred.
//
export const ERROR_DS_DECODING_ERROR = 8253;

//
// MessageId: ERROR_DS_FILTER_UNKNOWN
//
// MessageText:
//
//  The search filter cannot be recognized.
//
export const ERROR_DS_FILTER_UNKNOWN = 8254;

//
// MessageId: ERROR_DS_PARAM_ERROR
//
// MessageText:
//
//  One or more parameters are illegal.
//
export const ERROR_DS_PARAM_ERROR = 8255;

//
// MessageId: ERROR_DS_NOT_SUPPORTED
//
// MessageText:
//
//  The specified method is not supported.
//
export const ERROR_DS_NOT_SUPPORTED = 8256;

//
// MessageId: ERROR_DS_NO_RESULTS_RETURNED
//
// MessageText:
//
//  No results were returned.
//
export const ERROR_DS_NO_RESULTS_RETURNED = 8257;

//
// MessageId: ERROR_DS_CONTROL_NOT_FOUND
//
// MessageText:
//
//  The specified control is not supported by the server.
//
export const ERROR_DS_CONTROL_NOT_FOUND = 8258;

//
// MessageId: ERROR_DS_CLIENT_LOOP
//
// MessageText:
//
//  A referral loop was detected by the client.
//
export const ERROR_DS_CLIENT_LOOP = 8259;

//
// MessageId: ERROR_DS_REFERRAL_LIMIT_EXCEEDED
//
// MessageText:
//
//  The preset referral limit was exceeded.
//
export const ERROR_DS_REFERRAL_LIMIT_EXCEEDED = 8260;

//
// MessageId: ERROR_DS_SORT_CONTROL_MISSING
//
// MessageText:
//
//  The search requires a SORT control.
//
export const ERROR_DS_SORT_CONTROL_MISSING = 8261;

//
// MessageId: ERROR_DS_OFFSET_RANGE_ERROR
//
// MessageText:
//
//  The search results exceed the offset range specified.
//
export const ERROR_DS_OFFSET_RANGE_ERROR = 8262;

//
// MessageId: ERROR_DS_ROOT_MUST_BE_NC
//
// MessageText:
//
//  The root object must be the head of a naming context. The root object cannot have an instantiated parent.
//
export const ERROR_DS_ROOT_MUST_BE_NC = 8301;

//
// MessageId: ERROR_DS_ADD_REPLICA_INHIBITED
//
// MessageText:
//
//  The add replica operation cannot be performed. The naming context must be writeable in order to create the replica.
//
export const ERROR_DS_ADD_REPLICA_INHIBITED = 8302;

//
// MessageId: ERROR_DS_ATT_NOT_DEF_IN_SCHEMA
//
// MessageText:
//
//  A reference to an attribute that is not defined in the schema occurred.
//
export const ERROR_DS_ATT_NOT_DEF_IN_SCHEMA = 8303;

//
// MessageId: ERROR_DS_MAX_OBJ_SIZE_EXCEEDED
//
// MessageText:
//
//  The maximum size of an object has been exceeded.
//
export const ERROR_DS_MAX_OBJ_SIZE_EXCEEDED = 8304;

//
// MessageId: ERROR_DS_OBJ_STRING_NAME_EXISTS
//
// MessageText:
//
//  An attempt was made to add an object to the directory with a name that is already in use.
//
export const ERROR_DS_OBJ_STRING_NAME_EXISTS = 8305;

//
// MessageId: ERROR_DS_NO_RDN_DEFINED_IN_SCHEMA
//
// MessageText:
//
//  An attempt was made to add an object of a class that does not have an RDN defined in the schema.
//
export const ERROR_DS_NO_RDN_DEFINED_IN_SCHEMA = 8306;

//
// MessageId: ERROR_DS_RDN_DOESNT_MATCH_SCHEMA
//
// MessageText:
//
//  An attempt was made to add an object using an RDN that is not the RDN defined in the schema.
//
export const ERROR_DS_RDN_DOESNT_MATCH_SCHEMA = 8307;

//
// MessageId: ERROR_DS_NO_REQUESTED_ATTS_FOUND
//
// MessageText:
//
//  None of the requested attributes were found on the objects.
//
export const ERROR_DS_NO_REQUESTED_ATTS_FOUND = 8308;

//
// MessageId: ERROR_DS_USER_BUFFER_TO_SMALL
//
// MessageText:
//
//  The user buffer is too small.
//
export const ERROR_DS_USER_BUFFER_TO_SMALL = 8309;

//
// MessageId: ERROR_DS_ATT_IS_NOT_ON_OBJ
//
// MessageText:
//
//  The attribute specified in the operation is not present on the object.
//
export const ERROR_DS_ATT_IS_NOT_ON_OBJ = 8310;

//
// MessageId: ERROR_DS_ILLEGAL_MOD_OPERATION
//
// MessageText:
//
//  Illegal modify operation. Some aspect of the modification is not permitted.
//
export const ERROR_DS_ILLEGAL_MOD_OPERATION = 8311;

//
// MessageId: ERROR_DS_OBJ_TOO_LARGE
//
// MessageText:
//
//  The specified object is too large.
//
export const ERROR_DS_OBJ_TOO_LARGE = 8312;

//
// MessageId: ERROR_DS_BAD_INSTANCE_TYPE
//
// MessageText:
//
//  The specified instance type is not valid.
//
export const ERROR_DS_BAD_INSTANCE_TYPE = 8313;

//
// MessageId: ERROR_DS_MASTERDSA_REQUIRED
//
// MessageText:
//
//  The operation must be performed at a master DSA.
//
export const ERROR_DS_MASTERDSA_REQUIRED = 8314;

//
// MessageId: ERROR_DS_OBJECT_CLASS_REQUIRED
//
// MessageText:
//
//  The object class attribute must be specified.
//
export const ERROR_DS_OBJECT_CLASS_REQUIRED = 8315;

//
// MessageId: ERROR_DS_MISSING_REQUIRED_ATT
//
// MessageText:
//
//  A required attribute is missing.
//
export const ERROR_DS_MISSING_REQUIRED_ATT = 8316;

//
// MessageId: ERROR_DS_ATT_NOT_DEF_FOR_CLASS
//
// MessageText:
//
//  An attempt was made to modify an object to include an attribute that is not legal for its class.
//
export const ERROR_DS_ATT_NOT_DEF_FOR_CLASS = 8317;

//
// MessageId: ERROR_DS_ATT_ALREADY_EXISTS
//
// MessageText:
//
//  The specified attribute is already present on the object.
//
export const ERROR_DS_ATT_ALREADY_EXISTS = 8318;

// 8319 unused
//
// MessageId: ERROR_DS_CANT_ADD_ATT_VALUES
//
// MessageText:
//
//  The specified attribute is not present, or has no values.
//
export const ERROR_DS_CANT_ADD_ATT_VALUES = 8320;

//
// MessageId: ERROR_DS_ATT_VAL_ALREADY_EXISTS
//
// MessageText:
//
//  The specified value already exists.
//
export const ERROR_DS_ATT_VAL_ALREADY_EXISTS = 8323;

//
// MessageId: ERROR_DS_CANT_REM_MISSING_ATT
//
// MessageText:
//
//  The attribute cannot be removed because it is not present on the object.
//
export const ERROR_DS_CANT_REM_MISSING_ATT = 8324;

//
// MessageId: ERROR_DS_CANT_REM_MISSING_ATT_VAL
//
// MessageText:
//
//  The attribute value cannot be removed because it is not present on the object.
//
export const ERROR_DS_CANT_REM_MISSING_ATT_VAL = 8325;

//
// MessageId: ERROR_DS_ROOT_CANT_BE_SUBREF
//
// MessageText:
//
//  The specified root object cannot be a subref.
//
export const ERROR_DS_ROOT_CANT_BE_SUBREF = 8326;

//
// MessageId: ERROR_DS_NO_CHAINING
//
// MessageText:
//
//  Chaining is not permitted.
//
export const ERROR_DS_NO_CHAINING = 8327;

//
// MessageId: ERROR_DS_NO_CHAINED_EVAL
//
// MessageText:
//
//  Chained evaluation is not permitted.
//
export const ERROR_DS_NO_CHAINED_EVAL = 8328;

//
// MessageId: ERROR_DS_NO_PARENT_OBJECT
//
// MessageText:
//
//  The operation could not be performed because the object's parent is either uninstantiated or deleted.
//
export const ERROR_DS_NO_PARENT_OBJECT = 8329;

//
// MessageId: ERROR_DS_PARENT_IS_AN_ALIAS
//
// MessageText:
//
//  Having a parent that is an alias is not permitted. Aliases are leaf objects.
//
export const ERROR_DS_PARENT_IS_AN_ALIAS = 8330;

//
// MessageId: ERROR_DS_CANT_MIX_MASTER_AND_REPS
//
// MessageText:
//
//  The object and parent must be of the same type, either both masters or both replicas.
//
export const ERROR_DS_CANT_MIX_MASTER_AND_REPS = 8331;

//
// MessageId: ERROR_DS_CHILDREN_EXIST
//
// MessageText:
//
//  The operation cannot be performed because child objects exist. This operation can only be performed on a leaf object.
//
export const ERROR_DS_CHILDREN_EXIST = 8332;

//
// MessageId: ERROR_DS_OBJ_NOT_FOUND
//
// MessageText:
//
//  Directory object not found.
//
export const ERROR_DS_OBJ_NOT_FOUND = 8333;

//
// MessageId: ERROR_DS_ALIASED_OBJ_MISSING
//
// MessageText:
//
//  The aliased object is missing.
//
export const ERROR_DS_ALIASED_OBJ_MISSING = 8334;

//
// MessageId: ERROR_DS_BAD_NAME_SYNTAX
//
// MessageText:
//
//  The object name has bad syntax.
//
export const ERROR_DS_BAD_NAME_SYNTAX = 8335;

//
// MessageId: ERROR_DS_ALIAS_POINTS_TO_ALIAS
//
// MessageText:
//
//  It is not permitted for an alias to refer to another alias.
//
export const ERROR_DS_ALIAS_POINTS_TO_ALIAS = 8336;

//
// MessageId: ERROR_DS_CANT_DEREF_ALIAS
//
// MessageText:
//
//  The alias cannot be dereferenced.
//
export const ERROR_DS_CANT_DEREF_ALIAS = 8337;

//
// MessageId: ERROR_DS_OUT_OF_SCOPE
//
// MessageText:
//
//  The operation is out of scope.
//
export const ERROR_DS_OUT_OF_SCOPE = 8338;

//
// MessageId: ERROR_DS_OBJECT_BEING_REMOVED
//
// MessageText:
//
//  The operation cannot continue because the object is in the process of being removed.
//
export const ERROR_DS_OBJECT_BEING_REMOVED = 8339;

//
// MessageId: ERROR_DS_CANT_DELETE_DSA_OBJ
//
// MessageText:
//
//  The DSA object cannot be deleted.
//
export const ERROR_DS_CANT_DELETE_DSA_OBJ = 8340;

//
// MessageId: ERROR_DS_GENERIC_ERROR
//
// MessageText:
//
//  A directory service error has occurred.
//
export const ERROR_DS_GENERIC_ERROR = 8341;

//
// MessageId: ERROR_DS_DSA_MUST_BE_INT_MASTER
//
// MessageText:
//
//  The operation can only be performed on an internal master DSA object.
//
export const ERROR_DS_DSA_MUST_BE_INT_MASTER = 8342;

//
// MessageId: ERROR_DS_CLASS_NOT_DSA
//
// MessageText:
//
//  The object must be of class DSA.
//
export const ERROR_DS_CLASS_NOT_DSA = 8343;

//
// MessageId: ERROR_DS_INSUFF_ACCESS_RIGHTS
//
// MessageText:
//
//  Insufficient access rights to perform the operation.
//
export const ERROR_DS_INSUFF_ACCESS_RIGHTS = 8344;

//
// MessageId: ERROR_DS_ILLEGAL_SUPERIOR
//
// MessageText:
//
//  The object cannot be added because the parent is not on the list of possible superiors.
//
export const ERROR_DS_ILLEGAL_SUPERIOR = 8345;

//
// MessageId: ERROR_DS_ATTRIBUTE_OWNED_BY_SAM
//
// MessageText:
//
//  Access to the attribute is not permitted because the attribute is owned by the Security Accounts Manager (SAM).
//
export const ERROR_DS_ATTRIBUTE_OWNED_BY_SAM = 8346;

//
// MessageId: ERROR_DS_NAME_TOO_MANY_PARTS
//
// MessageText:
//
//  The name has too many parts.
//
export const ERROR_DS_NAME_TOO_MANY_PARTS = 8347;

//
// MessageId: ERROR_DS_NAME_TOO_LONG
//
// MessageText:
//
//  The name is too long.
//
export const ERROR_DS_NAME_TOO_LONG = 8348;

//
// MessageId: ERROR_DS_NAME_VALUE_TOO_LONG
//
// MessageText:
//
//  The name value is too long.
//
export const ERROR_DS_NAME_VALUE_TOO_LONG = 8349;

//
// MessageId: ERROR_DS_NAME_UNPARSEABLE
//
// MessageText:
//
//  The directory service encountered an error parsing a name.
//
export const ERROR_DS_NAME_UNPARSEABLE = 8350;

//
// MessageId: ERROR_DS_NAME_TYPE_UNKNOWN
//
// MessageText:
//
//  The directory service cannot get the attribute type for a name.
//
export const ERROR_DS_NAME_TYPE_UNKNOWN = 8351;

//
// MessageId: ERROR_DS_NOT_AN_OBJECT
//
// MessageText:
//
//  The name does not identify an object; the name identifies a phantom.
//
export const ERROR_DS_NOT_AN_OBJECT = 8352;

//
// MessageId: ERROR_DS_SEC_DESC_TOO_SHORT
//
// MessageText:
//
//  The security descriptor is too short.
//
export const ERROR_DS_SEC_DESC_TOO_SHORT = 8353;

//
// MessageId: ERROR_DS_SEC_DESC_INVALID
//
// MessageText:
//
//  The security descriptor is invalid.
//
export const ERROR_DS_SEC_DESC_INVALID = 8354;

//
// MessageId: ERROR_DS_NO_DELETED_NAME
//
// MessageText:
//
//  Failed to create name for deleted object.
//
export const ERROR_DS_NO_DELETED_NAME = 8355;

//
// MessageId: ERROR_DS_SUBREF_MUST_HAVE_PARENT
//
// MessageText:
//
//  The parent of a new subref must exist.
//
export const ERROR_DS_SUBREF_MUST_HAVE_PARENT = 8356;

//
// MessageId: ERROR_DS_NCNAME_MUST_BE_NC
//
// MessageText:
//
//  The object must be a naming context.
//
export const ERROR_DS_NCNAME_MUST_BE_NC = 8357;

//
// MessageId: ERROR_DS_CANT_ADD_SYSTEM_ONLY
//
// MessageText:
//
//  It is not permitted to add an attribute which is owned by the system.
//
export const ERROR_DS_CANT_ADD_SYSTEM_ONLY = 8358;

//
// MessageId: ERROR_DS_CLASS_MUST_BE_CONCRETE
//
// MessageText:
//
//  The class of the object must be structural; you cannot instantiate an abstract class.
//
export const ERROR_DS_CLASS_MUST_BE_CONCRETE = 8359;

//
// MessageId: ERROR_DS_INVALID_DMD
//
// MessageText:
//
//  The schema object could not be found.
//
export const ERROR_DS_INVALID_DMD = 8360;

//
// MessageId: ERROR_DS_OBJ_GUID_EXISTS
//
// MessageText:
//
//  A local object with this GUID (dead or alive) already exists.
//
export const ERROR_DS_OBJ_GUID_EXISTS = 8361;

//
// MessageId: ERROR_DS_NOT_ON_BACKLINK
//
// MessageText:
//
//  The operation cannot be performed on a back link.
//
export const ERROR_DS_NOT_ON_BACKLINK = 8362;

//
// MessageId: ERROR_DS_NO_CROSSREF_FOR_NC
//
// MessageText:
//
//  The cross reference for the specified naming context could not be found.
//
export const ERROR_DS_NO_CROSSREF_FOR_NC = 8363;

//
// MessageId: ERROR_DS_SHUTTING_DOWN
//
// MessageText:
//
//  The operation could not be performed because the directory service is shutting down.
//
export const ERROR_DS_SHUTTING_DOWN = 8364;

//
// MessageId: ERROR_DS_UNKNOWN_OPERATION
//
// MessageText:
//
//  The directory service request is invalid.
//
export const ERROR_DS_UNKNOWN_OPERATION = 8365;

//
// MessageId: ERROR_DS_INVALID_ROLE_OWNER
//
// MessageText:
//
//  The role owner attribute could not be read.
//
export const ERROR_DS_INVALID_ROLE_OWNER = 8366;

//
// MessageId: ERROR_DS_COULDNT_CONTACT_FSMO
//
// MessageText:
//
//  The requested FSMO operation failed. The current FSMO holder could not be contacted.
//
export const ERROR_DS_COULDNT_CONTACT_FSMO = 8367;

//
// MessageId: ERROR_DS_CROSS_NC_DN_RENAME
//
// MessageText:
//
//  Modification of a DN across a naming context is not permitted.
//
export const ERROR_DS_CROSS_NC_DN_RENAME = 8368;

//
// MessageId: ERROR_DS_CANT_MOD_SYSTEM_ONLY
//
// MessageText:
//
//  The attribute cannot be modified because it is owned by the system.
//
export const ERROR_DS_CANT_MOD_SYSTEM_ONLY = 8369;

//
// MessageId: ERROR_DS_REPLICATOR_ONLY
//
// MessageText:
//
//  Only the replicator can perform this function.
//
export const ERROR_DS_REPLICATOR_ONLY = 8370;

//
// MessageId: ERROR_DS_OBJ_CLASS_NOT_DEFINED
//
// MessageText:
//
//  The specified class is not defined.
//
export const ERROR_DS_OBJ_CLASS_NOT_DEFINED = 8371;

//
// MessageId: ERROR_DS_OBJ_CLASS_NOT_SUBCLASS
//
// MessageText:
//
//  The specified class is not a subclass.
//
export const ERROR_DS_OBJ_CLASS_NOT_SUBCLASS = 8372;

//
// MessageId: ERROR_DS_NAME_REFERENCE_INVALID
//
// MessageText:
//
//  The name reference is invalid.
//
export const ERROR_DS_NAME_REFERENCE_INVALID = 8373;

//
// MessageId: ERROR_DS_CROSS_REF_EXISTS
//
// MessageText:
//
//  A cross reference already exists.
//
export const ERROR_DS_CROSS_REF_EXISTS = 8374;

//
// MessageId: ERROR_DS_CANT_DEL_MASTER_CROSSREF
//
// MessageText:
//
//  It is not permitted to delete a master cross reference.
//
export const ERROR_DS_CANT_DEL_MASTER_CROSSREF = 8375;

//
// MessageId: ERROR_DS_SUBTREE_NOTIFY_NOT_NC_HEAD
//
// MessageText:
//
//  Subtree notifications are only supported on NC heads.
//
export const ERROR_DS_SUBTREE_NOTIFY_NOT_NC_HEAD = 8376;

//
// MessageId: ERROR_DS_NOTIFY_FILTER_TOO_COMPLEX
//
// MessageText:
//
//  Notification filter is too complex.
//
export const ERROR_DS_NOTIFY_FILTER_TOO_COMPLEX = 8377;

//
// MessageId: ERROR_DS_DUP_RDN
//
// MessageText:
//
//  Schema update failed: duplicate RDN.
//
export const ERROR_DS_DUP_RDN = 8378;

//
// MessageId: ERROR_DS_DUP_OID
//
// MessageText:
//
//  Schema update failed: duplicate OID.
//
export const ERROR_DS_DUP_OID = 8379;

//
// MessageId: ERROR_DS_DUP_MAPI_ID
//
// MessageText:
//
//  Schema update failed: duplicate MAPI identifier.
//
export const ERROR_DS_DUP_MAPI_ID = 8380;

//
// MessageId: ERROR_DS_DUP_SCHEMA_ID_GUID
//
// MessageText:
//
//  Schema update failed: duplicate schema-id GUID.
//
export const ERROR_DS_DUP_SCHEMA_ID_GUID = 8381;

//
// MessageId: ERROR_DS_DUP_LDAP_DISPLAY_NAME
//
// MessageText:
//
//  Schema update failed: duplicate LDAP display name.
//
export const ERROR_DS_DUP_LDAP_DISPLAY_NAME = 8382;

//
// MessageId: ERROR_DS_SEMANTIC_ATT_TEST
//
// MessageText:
//
//  Schema update failed: range-lower less than range upper.
//
export const ERROR_DS_SEMANTIC_ATT_TEST = 8383;

//
// MessageId: ERROR_DS_SYNTAX_MISMATCH
//
// MessageText:
//
//  Schema update failed: syntax mismatch.
//
export const ERROR_DS_SYNTAX_MISMATCH = 8384;

//
// MessageId: ERROR_DS_EXISTS_IN_MUST_HAVE
//
// MessageText:
//
//  Schema deletion failed: attribute is used in must-contain.
//
export const ERROR_DS_EXISTS_IN_MUST_HAVE = 8385;

//
// MessageId: ERROR_DS_EXISTS_IN_MAY_HAVE
//
// MessageText:
//
//  Schema deletion failed: attribute is used in may-contain.
//
export const ERROR_DS_EXISTS_IN_MAY_HAVE = 8386;

//
// MessageId: ERROR_DS_NONEXISTENT_MAY_HAVE
//
// MessageText:
//
//  Schema update failed: attribute in may-contain does not exist.
//
export const ERROR_DS_NONEXISTENT_MAY_HAVE = 8387;

//
// MessageId: ERROR_DS_NONEXISTENT_MUST_HAVE
//
// MessageText:
//
//  Schema update failed: attribute in must-contain does not exist.
//
export const ERROR_DS_NONEXISTENT_MUST_HAVE = 8388;

//
// MessageId: ERROR_DS_AUX_CLS_TEST_FAIL
//
// MessageText:
//
//  Schema update failed: class in aux-class list does not exist or is not an auxiliary class.
//
export const ERROR_DS_AUX_CLS_TEST_FAIL = 8389;

//
// MessageId: ERROR_DS_NONEXISTENT_POSS_SUP
//
// MessageText:
//
//  Schema update failed: class in poss-superiors does not exist.
//
export const ERROR_DS_NONEXISTENT_POSS_SUP = 8390;

//
// MessageId: ERROR_DS_SUB_CLS_TEST_FAIL
//
// MessageText:
//
//  Schema update failed: class in subclassof list does not exist or does not satisfy hierarchy rules.
//
export const ERROR_DS_SUB_CLS_TEST_FAIL = 8391;

//
// MessageId: ERROR_DS_BAD_RDN_ATT_ID_SYNTAX
//
// MessageText:
//
//  Schema update failed: Rdn-Att-Id has wrong syntax.
//
export const ERROR_DS_BAD_RDN_ATT_ID_SYNTAX = 8392;

//
// MessageId: ERROR_DS_EXISTS_IN_AUX_CLS
//
// MessageText:
//
//  Schema deletion failed: class is used as auxiliary class.
//
export const ERROR_DS_EXISTS_IN_AUX_CLS = 8393;

//
// MessageId: ERROR_DS_EXISTS_IN_SUB_CLS
//
// MessageText:
//
//  Schema deletion failed: class is used as sub class.
//
export const ERROR_DS_EXISTS_IN_SUB_CLS = 8394;

//
// MessageId: ERROR_DS_EXISTS_IN_POSS_SUP
//
// MessageText:
//
//  Schema deletion failed: class is used as poss-superior.
//
export const ERROR_DS_EXISTS_IN_POSS_SUP = 8395;

//
// MessageId: ERROR_DS_RECALCSCHEMA_FAILED
//
// MessageText:
//
//  Schema update failed in recalculating validation cache.
//
export const ERROR_DS_RECALCSCHEMA_FAILED = 8396;

//
// MessageId: ERROR_DS_TREE_DELETE_NOT_FINISHED
//
// MessageText:
//
//  The tree deletion is not finished.  The request must be made again to continue deleting the tree.
//
export const ERROR_DS_TREE_DELETE_NOT_FINISHED = 8397;

//
// MessageId: ERROR_DS_CANT_DELETE
//
// MessageText:
//
//  The requested delete operation could not be performed.
//
export const ERROR_DS_CANT_DELETE = 8398;

//
// MessageId: ERROR_DS_ATT_SCHEMA_REQ_ID
//
// MessageText:
//
//  Cannot read the governs class identifier for the schema record.
//
export const ERROR_DS_ATT_SCHEMA_REQ_ID = 8399;

//
// MessageId: ERROR_DS_BAD_ATT_SCHEMA_SYNTAX
//
// MessageText:
//
//  The attribute schema has bad syntax.
//
export const ERROR_DS_BAD_ATT_SCHEMA_SYNTAX = 8400;

//
// MessageId: ERROR_DS_CANT_CACHE_ATT
//
// MessageText:
//
//  The attribute could not be cached.
//
export const ERROR_DS_CANT_CACHE_ATT = 8401;

//
// MessageId: ERROR_DS_CANT_CACHE_CLASS
//
// MessageText:
//
//  The class could not be cached.
//
export const ERROR_DS_CANT_CACHE_CLASS = 8402;

//
// MessageId: ERROR_DS_CANT_REMOVE_ATT_CACHE
//
// MessageText:
//
//  The attribute could not be removed from the cache.
//
export const ERROR_DS_CANT_REMOVE_ATT_CACHE = 8403;

//
// MessageId: ERROR_DS_CANT_REMOVE_CLASS_CACHE
//
// MessageText:
//
//  The class could not be removed from the cache.
//
export const ERROR_DS_CANT_REMOVE_CLASS_CACHE = 8404;

//
// MessageId: ERROR_DS_CANT_RETRIEVE_DN
//
// MessageText:
//
//  The distinguished name attribute could not be read.
//
export const ERROR_DS_CANT_RETRIEVE_DN = 8405;

//
// MessageId: ERROR_DS_MISSING_SUPREF
//
// MessageText:
//
//  No superior reference has been configured for the directory service. The directory service is therefore unable to issue referrals to objects outside this forest.
//
export const ERROR_DS_MISSING_SUPREF = 8406;

//
// MessageId: ERROR_DS_CANT_RETRIEVE_INSTANCE
//
// MessageText:
//
//  The instance type attribute could not be retrieved.
//
export const ERROR_DS_CANT_RETRIEVE_INSTANCE = 8407;

//
// MessageId: ERROR_DS_CODE_INCONSISTENCY
//
// MessageText:
//
//  An internal error has occurred.
//
export const ERROR_DS_CODE_INCONSISTENCY = 8408;

//
// MessageId: ERROR_DS_DATABASE_ERROR
//
// MessageText:
//
//  A database error has occurred.
//
export const ERROR_DS_DATABASE_ERROR = 8409;

//
// MessageId: ERROR_DS_GOVERNSID_MISSING
//
// MessageText:
//
//  The attribute GOVERNSID is missing.
//
export const ERROR_DS_GOVERNSID_MISSING = 8410;

//
// MessageId: ERROR_DS_MISSING_EXPECTED_ATT
//
// MessageText:
//
//  An expected attribute is missing.
//
export const ERROR_DS_MISSING_EXPECTED_ATT = 8411;

//
// MessageId: ERROR_DS_NCNAME_MISSING_CR_REF
//
// MessageText:
//
//  The specified naming context is missing a cross reference.
//
export const ERROR_DS_NCNAME_MISSING_CR_REF = 8412;

//
// MessageId: ERROR_DS_SECURITY_CHECKING_ERROR
//
// MessageText:
//
//  A security checking error has occurred.
//
export const ERROR_DS_SECURITY_CHECKING_ERROR = 8413;

//
// MessageId: ERROR_DS_SCHEMA_NOT_LOADED
//
// MessageText:
//
//  The schema is not loaded.
//
export const ERROR_DS_SCHEMA_NOT_LOADED = 8414;

//
// MessageId: ERROR_DS_SCHEMA_ALLOC_FAILED
//
// MessageText:
//
//  Schema allocation failed. Please check if the machine is running low on memory.
//
export const ERROR_DS_SCHEMA_ALLOC_FAILED = 8415;

//
// MessageId: ERROR_DS_ATT_SCHEMA_REQ_SYNTAX
//
// MessageText:
//
//  Failed to obtain the required syntax for the attribute schema.
//
export const ERROR_DS_ATT_SCHEMA_REQ_SYNTAX = 8416;

//
// MessageId: ERROR_DS_GCVERIFY_ERROR
//
// MessageText:
//
//  The global catalog verification failed. The global catalog is not available or does not support the operation. Some part of the directory is currently not available.
//
export const ERROR_DS_GCVERIFY_ERROR = 8417;

//
// MessageId: ERROR_DS_DRA_SCHEMA_MISMATCH
//
// MessageText:
//
//  The replication operation failed because of a schema mismatch between the servers involved.
//
export const ERROR_DS_DRA_SCHEMA_MISMATCH = 8418;

//
// MessageId: ERROR_DS_CANT_FIND_DSA_OBJ
//
// MessageText:
//
//  The DSA object could not be found.
//
export const ERROR_DS_CANT_FIND_DSA_OBJ = 8419;

//
// MessageId: ERROR_DS_CANT_FIND_EXPECTED_NC
//
// MessageText:
//
//  The naming context could not be found.
//
export const ERROR_DS_CANT_FIND_EXPECTED_NC = 8420;

//
// MessageId: ERROR_DS_CANT_FIND_NC_IN_CACHE
//
// MessageText:
//
//  The naming context could not be found in the cache.
//
export const ERROR_DS_CANT_FIND_NC_IN_CACHE = 8421;

//
// MessageId: ERROR_DS_CANT_RETRIEVE_CHILD
//
// MessageText:
//
//  The child object could not be retrieved.
//
export const ERROR_DS_CANT_RETRIEVE_CHILD = 8422;

//
// MessageId: ERROR_DS_SECURITY_ILLEGAL_MODIFY
//
// MessageText:
//
//  The modification was not permitted for security reasons.
//
export const ERROR_DS_SECURITY_ILLEGAL_MODIFY = 8423;

//
// MessageId: ERROR_DS_CANT_REPLACE_HIDDEN_REC
//
// MessageText:
//
//  The operation cannot replace the hidden record.
//
export const ERROR_DS_CANT_REPLACE_HIDDEN_REC = 8424;

//
// MessageId: ERROR_DS_BAD_HIERARCHY_FILE
//
// MessageText:
//
//  The hierarchy file is invalid.
//
export const ERROR_DS_BAD_HIERARCHY_FILE = 8425;

//
// MessageId: ERROR_DS_BUILD_HIERARCHY_TABLE_FAILED
//
// MessageText:
//
//  The attempt to build the hierarchy table failed.
//
export const ERROR_DS_BUILD_HIERARCHY_TABLE_FAILED = 8426;

//
// MessageId: ERROR_DS_CONFIG_PARAM_MISSING
//
// MessageText:
//
//  The directory configuration parameter is missing from the registry.
//
export const ERROR_DS_CONFIG_PARAM_MISSING = 8427;

//
// MessageId: ERROR_DS_COUNTING_AB_INDICES_FAILED
//
// MessageText:
//
//  The attempt to count the address book indices failed.
//
export const ERROR_DS_COUNTING_AB_INDICES_FAILED = 8428;

//
// MessageId: ERROR_DS_HIERARCHY_TABLE_MALLOC_FAILED
//
// MessageText:
//
//  The allocation of the hierarchy table failed.
//
export const ERROR_DS_HIERARCHY_TABLE_MALLOC_FAILED = 8429;

//
// MessageId: ERROR_DS_INTERNAL_FAILURE
//
// MessageText:
//
//  The directory service encountered an internal failure.
//
export const ERROR_DS_INTERNAL_FAILURE = 8430;

//
// MessageId: ERROR_DS_UNKNOWN_ERROR
//
// MessageText:
//
//  The directory service encountered an unknown failure.
//
export const ERROR_DS_UNKNOWN_ERROR = 8431;

//
// MessageId: ERROR_DS_ROOT_REQUIRES_CLASS_TOP
//
// MessageText:
//
//  A root object requires a class of 'top'.
//
export const ERROR_DS_ROOT_REQUIRES_CLASS_TOP = 8432;

//
// MessageId: ERROR_DS_REFUSING_FSMO_ROLES
//
// MessageText:
//
//  This directory server is shutting down, and cannot take ownership of new floating single-master operation roles.
//
export const ERROR_DS_REFUSING_FSMO_ROLES = 8433;

//
// MessageId: ERROR_DS_MISSING_FSMO_SETTINGS
//
// MessageText:
//
//  The directory service is missing mandatory configuration information, and is unable to determine the ownership of floating single-master operation roles.
//
export const ERROR_DS_MISSING_FSMO_SETTINGS = 8434;

//
// MessageId: ERROR_DS_UNABLE_TO_SURRENDER_ROLES
//
// MessageText:
//
//  The directory service was unable to transfer ownership of one or more floating single-master operation roles to other servers.
//
export const ERROR_DS_UNABLE_TO_SURRENDER_ROLES = 8435;

//
// MessageId: ERROR_DS_DRA_GENERIC
//
// MessageText:
//
//  The replication operation failed.
//
export const ERROR_DS_DRA_GENERIC = 8436;

//
// MessageId: ERROR_DS_DRA_INVALID_PARAMETER
//
// MessageText:
//
//  An invalid parameter was specified for this replication operation.
//
export const ERROR_DS_DRA_INVALID_PARAMETER = 8437;

//
// MessageId: ERROR_DS_DRA_BUSY
//
// MessageText:
//
//  The directory service is too busy to complete the replication operation at this time.
//
export const ERROR_DS_DRA_BUSY = 8438;

//
// MessageId: ERROR_DS_DRA_BAD_DN
//
// MessageText:
//
//  The distinguished name specified for this replication operation is invalid.
//
export const ERROR_DS_DRA_BAD_DN = 8439;

//
// MessageId: ERROR_DS_DRA_BAD_NC
//
// MessageText:
//
//  The naming context specified for this replication operation is invalid.
//
export const ERROR_DS_DRA_BAD_NC = 8440;

//
// MessageId: ERROR_DS_DRA_DN_EXISTS
//
// MessageText:
//
//  The distinguished name specified for this replication operation already exists.
//
export const ERROR_DS_DRA_DN_EXISTS = 8441;

//
// MessageId: ERROR_DS_DRA_INTERNAL_ERROR
//
// MessageText:
//
//  The replication system encountered an internal error.
//
export const ERROR_DS_DRA_INTERNAL_ERROR = 8442;

//
// MessageId: ERROR_DS_DRA_INCONSISTENT_DIT
//
// MessageText:
//
//  The replication operation encountered a database inconsistency.
//
export const ERROR_DS_DRA_INCONSISTENT_DIT = 8443;

//
// MessageId: ERROR_DS_DRA_CONNECTION_FAILED
//
// MessageText:
//
//  The server specified for this replication operation could not be contacted.
//
export const ERROR_DS_DRA_CONNECTION_FAILED = 8444;

//
// MessageId: ERROR_DS_DRA_BAD_INSTANCE_TYPE
//
// MessageText:
//
//  The replication operation encountered an object with an invalid instance type.
//
export const ERROR_DS_DRA_BAD_INSTANCE_TYPE = 8445;

//
// MessageId: ERROR_DS_DRA_OUT_OF_MEM
//
// MessageText:
//
//  The replication operation failed to allocate memory.
//
export const ERROR_DS_DRA_OUT_OF_MEM = 8446;

//
// MessageId: ERROR_DS_DRA_MAIL_PROBLEM
//
// MessageText:
//
//  The replication operation encountered an error with the mail system.
//
export const ERROR_DS_DRA_MAIL_PROBLEM = 8447;

//
// MessageId: ERROR_DS_DRA_REF_ALREADY_EXISTS
//
// MessageText:
//
//  The replication reference information for the target server already exists.
//
export const ERROR_DS_DRA_REF_ALREADY_EXISTS = 8448;

//
// MessageId: ERROR_DS_DRA_REF_NOT_FOUND
//
// MessageText:
//
//  The replication reference information for the target server does not exist.
//
export const ERROR_DS_DRA_REF_NOT_FOUND = 8449;

//
// MessageId: ERROR_DS_DRA_OBJ_IS_REP_SOURCE
//
// MessageText:
//
//  The naming context cannot be removed because it is replicated to another server.
//
export const ERROR_DS_DRA_OBJ_IS_REP_SOURCE = 8450;

//
// MessageId: ERROR_DS_DRA_DB_ERROR
//
// MessageText:
//
//  The replication operation encountered a database error.
//
export const ERROR_DS_DRA_DB_ERROR = 8451;

//
// MessageId: ERROR_DS_DRA_NO_REPLICA
//
// MessageText:
//
//  The naming context is in the process of being removed or is not replicated from the specified server.
//
export const ERROR_DS_DRA_NO_REPLICA = 8452;

//
// MessageId: ERROR_DS_DRA_ACCESS_DENIED
//
// MessageText:
//
//  Replication access was denied.
//
export const ERROR_DS_DRA_ACCESS_DENIED = 8453;

//
// MessageId: ERROR_DS_DRA_NOT_SUPPORTED
//
// MessageText:
//
//  The requested operation is not supported by this version of the directory service.
//
export const ERROR_DS_DRA_NOT_SUPPORTED = 8454;

//
// MessageId: ERROR_DS_DRA_RPC_CANCELLED
//
// MessageText:
//
//  The replication remote procedure call was cancelled.
//
export const ERROR_DS_DRA_RPC_CANCELLED = 8455;

//
// MessageId: ERROR_DS_DRA_SOURCE_DISABLED
//
// MessageText:
//
//  The source server is currently rejecting replication requests.
//
export const ERROR_DS_DRA_SOURCE_DISABLED = 8456;

//
// MessageId: ERROR_DS_DRA_SINK_DISABLED
//
// MessageText:
//
//  The destination server is currently rejecting replication requests.
//
export const ERROR_DS_DRA_SINK_DISABLED = 8457;

//
// MessageId: ERROR_DS_DRA_NAME_COLLISION
//
// MessageText:
//
//  The replication operation failed due to a collision of object names.
//
export const ERROR_DS_DRA_NAME_COLLISION = 8458;

//
// MessageId: ERROR_DS_DRA_SOURCE_REINSTALLED
//
// MessageText:
//
//  The replication source has been reinstalled.
//
export const ERROR_DS_DRA_SOURCE_REINSTALLED = 8459;

//
// MessageId: ERROR_DS_DRA_MISSING_PARENT
//
// MessageText:
//
//  The replication operation failed because a required parent object is missing.
//
export const ERROR_DS_DRA_MISSING_PARENT = 8460;

//
// MessageId: ERROR_DS_DRA_PREEMPTED
//
// MessageText:
//
//  The replication operation was preempted.
//
export const ERROR_DS_DRA_PREEMPTED = 8461;

//
// MessageId: ERROR_DS_DRA_ABANDON_SYNC
//
// MessageText:
//
//  The replication synchronization attempt was abandoned because of a lack of updates.
//
export const ERROR_DS_DRA_ABANDON_SYNC = 8462;

//
// MessageId: ERROR_DS_DRA_SHUTDOWN
//
// MessageText:
//
//  The replication operation was terminated because the system is shutting down.
//
export const ERROR_DS_DRA_SHUTDOWN = 8463;

//
// MessageId: ERROR_DS_DRA_INCOMPATIBLE_PARTIAL_SET
//
// MessageText:
//
//  Synchronization attempt failed because the destination DC is currently waiting to synchronize new partial attributes from source. This condition is normal if a recent schema change modified the partial attribute set. The destination partial attribute set is not a subset of source partial attribute set.
//
export const ERROR_DS_DRA_INCOMPATIBLE_PARTIAL_SET = 8464;

//
// MessageId: ERROR_DS_DRA_SOURCE_IS_PARTIAL_REPLICA
//
// MessageText:
//
//  The replication synchronization attempt failed because a master replica attempted to sync from a partial replica.
//
export const ERROR_DS_DRA_SOURCE_IS_PARTIAL_REPLICA = 8465;

//
// MessageId: ERROR_DS_DRA_EXTN_CONNECTION_FAILED
//
// MessageText:
//
//  The server specified for this replication operation was contacted, but that server was unable to contact an additional server needed to complete the operation.
//
export const ERROR_DS_DRA_EXTN_CONNECTION_FAILED = 8466;

//
// MessageId: ERROR_DS_INSTALL_SCHEMA_MISMATCH
//
// MessageText:
//
//  The version of the Active Directory schema of the source forest is not compatible with the version of Active Directory on this computer.
//
export const ERROR_DS_INSTALL_SCHEMA_MISMATCH = 8467;

//
// MessageId: ERROR_DS_DUP_LINK_ID
//
// MessageText:
//
//  Schema update failed: An attribute with the same link identifier already exists.
//
export const ERROR_DS_DUP_LINK_ID = 8468;

//
// MessageId: ERROR_DS_NAME_ERROR_RESOLVING
//
// MessageText:
//
//  Name translation: Generic processing error.
//
export const ERROR_DS_NAME_ERROR_RESOLVING = 8469;

//
// MessageId: ERROR_DS_NAME_ERROR_NOT_FOUND
//
// MessageText:
//
//  Name translation: Could not find the name or insufficient right to see name.
//
export const ERROR_DS_NAME_ERROR_NOT_FOUND = 8470;

//
// MessageId: ERROR_DS_NAME_ERROR_NOT_UNIQUE
//
// MessageText:
//
//  Name translation: Input name mapped to more than one output name.
//
export const ERROR_DS_NAME_ERROR_NOT_UNIQUE = 8471;

//
// MessageId: ERROR_DS_NAME_ERROR_NO_MAPPING
//
// MessageText:
//
//  Name translation: Input name found, but not the associated output format.
//
export const ERROR_DS_NAME_ERROR_NO_MAPPING = 8472;

//
// MessageId: ERROR_DS_NAME_ERROR_DOMAIN_ONLY
//
// MessageText:
//
//  Name translation: Unable to resolve completely, only the domain was found.
//
export const ERROR_DS_NAME_ERROR_DOMAIN_ONLY = 8473;

//
// MessageId: ERROR_DS_NAME_ERROR_NO_SYNTACTICAL_MAPPING
//
// MessageText:
//
//  Name translation: Unable to perform purely syntactical mapping at the client without going out to the wire.
//
export const ERROR_DS_NAME_ERROR_NO_SYNTACTICAL_MAPPING = 8474;

//
// MessageId: ERROR_DS_WRONG_OM_OBJ_CLASS
//
// MessageText:
//
//  The OM-Object-Class specified is incorrect for an attribute with the specified syntax.
//
export const ERROR_DS_WRONG_OM_OBJ_CLASS = 8476;

//
// MessageId: ERROR_DS_DRA_REPL_PENDING
//
// MessageText:
//
//  The replication request has been posted; waiting for reply.
//
export const ERROR_DS_DRA_REPL_PENDING = 8477;

//
// MessageId: ERROR_DS_DS_REQUIRED
//
// MessageText:
//
//  The requested operation requires a directory service, and none was available.
//
export const ERROR_DS_DS_REQUIRED = 8478;

//
// MessageId: ERROR_DS_INVALID_LDAP_DISPLAY_NAME
//
// MessageText:
//
//  The LDAP display name of the class or attribute contains non-ASCII characters.
//
export const ERROR_DS_INVALID_LDAP_DISPLAY_NAME = 8479;

//
// MessageId: ERROR_DS_NON_BASE_SEARCH
//
// MessageText:
//
//  The requested search operation is only supported for base searches.
//
export const ERROR_DS_NON_BASE_SEARCH = 8480;

//
// MessageId: ERROR_DS_CANT_RETRIEVE_ATTS
//
// MessageText:
//
//  The search failed to retrieve attributes from the database.
//
export const ERROR_DS_CANT_RETRIEVE_ATTS = 8481;

//
// MessageId: ERROR_DS_BACKLINK_WITHOUT_LINK
//
// MessageText:
//
//  The schema update operation tried to add a backward link attribute that has no corresponding forward link.
//
export const ERROR_DS_BACKLINK_WITHOUT_LINK = 8482;

//
// MessageId: ERROR_DS_EPOCH_MISMATCH
//
// MessageText:
//
//  Source and destination of a cross-domain move do not agree on the object's epoch number.  Either source or destination does not have the latest version of the object.
//
export const ERROR_DS_EPOCH_MISMATCH = 8483;

//
// MessageId: ERROR_DS_SRC_NAME_MISMATCH
//
// MessageText:
//
//  Source and destination of a cross-domain move do not agree on the object's current name.  Either source or destination does not have the latest version of the object.
//
export const ERROR_DS_SRC_NAME_MISMATCH = 8484;

//
// MessageId: ERROR_DS_SRC_AND_DST_NC_IDENTICAL
//
// MessageText:
//
//  Source and destination for the cross-domain move operation are identical.  Caller should use local move operation instead of cross-domain move operation.
//
export const ERROR_DS_SRC_AND_DST_NC_IDENTICAL = 8485;

//
// MessageId: ERROR_DS_DST_NC_MISMATCH
//
// MessageText:
//
//  Source and destination for a cross-domain move are not in agreement on the naming contexts in the forest.  Either source or destination does not have the latest version of the Partitions container.
//
export const ERROR_DS_DST_NC_MISMATCH = 8486;

//
// MessageId: ERROR_DS_NOT_AUTHORITIVE_FOR_DST_NC
//
// MessageText:
//
//  Destination of a cross-domain move is not authoritative for the destination naming context.
//
export const ERROR_DS_NOT_AUTHORITIVE_FOR_DST_NC = 8487;

//
// MessageId: ERROR_DS_SRC_GUID_MISMATCH
//
// MessageText:
//
//  Source and destination of a cross-domain move do not agree on the identity of the source object.  Either source or destination does not have the latest version of the source object.
//
export const ERROR_DS_SRC_GUID_MISMATCH = 8488;

//
// MessageId: ERROR_DS_CANT_MOVE_DELETED_OBJECT
//
// MessageText:
//
//  Object being moved across-domains is already known to be deleted by the destination server.  The source server does not have the latest version of the source object.
//
export const ERROR_DS_CANT_MOVE_DELETED_OBJECT = 8489;

//
// MessageId: ERROR_DS_PDC_OPERATION_IN_PROGRESS
//
// MessageText:
//
//  Another operation which requires exclusive access to the PDC FSMO is already in progress.
//
export const ERROR_DS_PDC_OPERATION_IN_PROGRESS = 8490;

//
// MessageId: ERROR_DS_CROSS_DOMAIN_CLEANUP_REQD
//
// MessageText:
//
//  A cross-domain move operation failed such that two versions of the moved object exist - one each in the source and destination domains.  The destination object needs to be removed to restore the system to a consistent state.
//
export const ERROR_DS_CROSS_DOMAIN_CLEANUP_REQD = 8491;

//
// MessageId: ERROR_DS_ILLEGAL_XDOM_MOVE_OPERATION
//
// MessageText:
//
//  This object may not be moved across domain boundaries either because cross-domain moves for this class are disallowed, or the object has some special characteristics, e.g.: trust account or restricted RID, which prevent its move.
//
export const ERROR_DS_ILLEGAL_XDOM_MOVE_OPERATION = 8492;

//
// MessageId: ERROR_DS_CANT_WITH_ACCT_GROUP_MEMBERSHPS
//
// MessageText:
//
//  Can't move objects with memberships across domain boundaries as once moved, this would violate the membership conditions of the account group.  Remove the object from any account group memberships and retry.
//
export const ERROR_DS_CANT_WITH_ACCT_GROUP_MEMBERSHPS = 8493;

//
// MessageId: ERROR_DS_NC_MUST_HAVE_NC_PARENT
//
// MessageText:
//
//  A naming context head must be the immediate child of another naming context head, not of an interior node.
//
export const ERROR_DS_NC_MUST_HAVE_NC_PARENT = 8494;

//
// MessageId: ERROR_DS_CR_IMPOSSIBLE_TO_VALIDATE
//
// MessageText:
//
//  The directory cannot validate the proposed naming context name because it does not hold a replica of the naming context above the proposed naming context.  Please ensure that the domain naming master role is held by a server that is configured as a global catalog server, and that the server is up to date with its replication partners. (Applies only to Windows 2000 Domain Naming masters)
//
export const ERROR_DS_CR_IMPOSSIBLE_TO_VALIDATE = 8495;

//
// MessageId: ERROR_DS_DST_DOMAIN_NOT_NATIVE
//
// MessageText:
//
//  Destination domain must be in native mode.
//
export const ERROR_DS_DST_DOMAIN_NOT_NATIVE = 8496;

//
// MessageId: ERROR_DS_MISSING_INFRASTRUCTURE_CONTAINER
//
// MessageText:
//
//  The operation can not be performed because the server does not have an infrastructure container in the domain of interest.
//
export const ERROR_DS_MISSING_INFRASTRUCTURE_CONTAINER = 8497;

//
// MessageId: ERROR_DS_CANT_MOVE_ACCOUNT_GROUP
//
// MessageText:
//
//  Cross-domain move of non-empty account groups is not allowed.
//
export const ERROR_DS_CANT_MOVE_ACCOUNT_GROUP = 8498;

//
// MessageId: ERROR_DS_CANT_MOVE_RESOURCE_GROUP
//
// MessageText:
//
//  Cross-domain move of non-empty resource groups is not allowed.
//
export const ERROR_DS_CANT_MOVE_RESOURCE_GROUP = 8499;

//
// MessageId: ERROR_DS_INVALID_SEARCH_FLAG
//
// MessageText:
//
//  The search flags for the attribute are invalid. The ANR bit is valid only on attributes of Unicode or Teletex strings.
//
export const ERROR_DS_INVALID_SEARCH_FLAG = 8500;

//
// MessageId: ERROR_DS_NO_TREE_DELETE_ABOVE_NC
//
// MessageText:
//
//  Tree deletions starting at an object which has an NC head as a descendant are not allowed.
//
export const ERROR_DS_NO_TREE_DELETE_ABOVE_NC = 8501;

//
// MessageId: ERROR_DS_COULDNT_LOCK_TREE_FOR_DELETE
//
// MessageText:
//
//  The directory service failed to lock a tree in preparation for a tree deletion because the tree was in use.
//
export const ERROR_DS_COULDNT_LOCK_TREE_FOR_DELETE = 8502;

//
// MessageId: ERROR_DS_COULDNT_IDENTIFY_OBJECTS_FOR_TREE_DELETE
//
// MessageText:
//
//  The directory service failed to identify the list of objects to delete while attempting a tree deletion.
//
export const ERROR_DS_COULDNT_IDENTIFY_OBJECTS_FOR_TREE_DELETE = 8503;

//
// MessageId: ERROR_DS_SAM_INIT_FAILURE
//
// MessageText:
//
//  Security Accounts Manager initialization failed because of the following error: %1.
//  Error Status: 0x%2. Click OK to shut down the system and reboot into Directory Services Restore Mode. Check the event log for detailed information.
//
export const ERROR_DS_SAM_INIT_FAILURE = 8504;

//
// MessageId: ERROR_DS_SENSITIVE_GROUP_VIOLATION
//
// MessageText:
//
//  Only an administrator can modify the membership list of an administrative group.
//
export const ERROR_DS_SENSITIVE_GROUP_VIOLATION = 8505;

//
// MessageId: ERROR_DS_CANT_MOD_PRIMARYGROUPID
//
// MessageText:
//
//  Cannot change the primary group ID of a domain controller account.
//
export const ERROR_DS_CANT_MOD_PRIMARYGROUPID = 8506;

//
// MessageId: ERROR_DS_ILLEGAL_BASE_SCHEMA_MOD
//
// MessageText:
//
//  An attempt is made to modify the base schema.
//
export const ERROR_DS_ILLEGAL_BASE_SCHEMA_MOD = 8507;

//
// MessageId: ERROR_DS_NONSAFE_SCHEMA_CHANGE
//
// MessageText:
//
//  Adding a new mandatory attribute to an existing class, deleting a mandatory attribute from an existing class, or adding an optional attribute to the special class Top that is not a backlink attribute (directly or through inheritance, for example, by adding or deleting an auxiliary class) is not allowed.
//
export const ERROR_DS_NONSAFE_SCHEMA_CHANGE = 8508;

//
// MessageId: ERROR_DS_SCHEMA_UPDATE_DISALLOWED
//
// MessageText:
//
//  Schema update is not allowed on this DC because the DC is not the schema FSMO Role Owner.
//
export const ERROR_DS_SCHEMA_UPDATE_DISALLOWED = 8509;

//
// MessageId: ERROR_DS_CANT_CREATE_UNDER_SCHEMA
//
// MessageText:
//
//  An object of this class cannot be created under the schema container. You can only create attribute-schema and class-schema objects under the schema container.
//
export const ERROR_DS_CANT_CREATE_UNDER_SCHEMA = 8510;

//
// MessageId: ERROR_DS_INSTALL_NO_SRC_SCH_VERSION
//
// MessageText:
//
//  The replica/child install failed to get the objectVersion attribute on the schema container on the source DC. Either the attribute is missing on the schema container or the credentials supplied do not have permission to read it.
//
export const ERROR_DS_INSTALL_NO_SRC_SCH_VERSION = 8511;

//
// MessageId: ERROR_DS_INSTALL_NO_SCH_VERSION_IN_INIFILE
//
// MessageText:
//
//  The replica/child install failed to read the objectVersion attribute in the SCHEMA section of the file schema.ini in the system32 directory.
//
export const ERROR_DS_INSTALL_NO_SCH_VERSION_IN_INIFILE = 8512;

//
// MessageId: ERROR_DS_INVALID_GROUP_TYPE
//
// MessageText:
//
//  The specified group type is invalid.
//
export const ERROR_DS_INVALID_GROUP_TYPE = 8513;

//
// MessageId: ERROR_DS_NO_NEST_GLOBALGROUP_IN_MIXEDDOMAIN
//
// MessageText:
//
//  You cannot nest global groups in a mixed domain if the group is security-enabled.
//
export const ERROR_DS_NO_NEST_GLOBALGROUP_IN_MIXEDDOMAIN = 8514;

//
// MessageId: ERROR_DS_NO_NEST_LOCALGROUP_IN_MIXEDDOMAIN
//
// MessageText:
//
//  You cannot nest local groups in a mixed domain if the group is security-enabled.
//
export const ERROR_DS_NO_NEST_LOCALGROUP_IN_MIXEDDOMAIN = 8515;

//
// MessageId: ERROR_DS_GLOBAL_CANT_HAVE_LOCAL_MEMBER
//
// MessageText:
//
//  A global group cannot have a local group as a member.
//
export const ERROR_DS_GLOBAL_CANT_HAVE_LOCAL_MEMBER = 8516;

//
// MessageId: ERROR_DS_GLOBAL_CANT_HAVE_UNIVERSAL_MEMBER
//
// MessageText:
//
//  A global group cannot have a universal group as a member.
//
export const ERROR_DS_GLOBAL_CANT_HAVE_UNIVERSAL_MEMBER = 8517;

//
// MessageId: ERROR_DS_UNIVERSAL_CANT_HAVE_LOCAL_MEMBER
//
// MessageText:
//
//  A universal group cannot have a local group as a member.
//
export const ERROR_DS_UNIVERSAL_CANT_HAVE_LOCAL_MEMBER = 8518;

//
// MessageId: ERROR_DS_GLOBAL_CANT_HAVE_CROSSDOMAIN_MEMBER
//
// MessageText:
//
//  A global group cannot have a cross-domain member.
//
export const ERROR_DS_GLOBAL_CANT_HAVE_CROSSDOMAIN_MEMBER = 8519;

//
// MessageId: ERROR_DS_LOCAL_CANT_HAVE_CROSSDOMAIN_LOCAL_MEMBER
//
// MessageText:
//
//  A local group cannot have another cross domain local group as a member.
//
export const ERROR_DS_LOCAL_CANT_HAVE_CROSSDOMAIN_LOCAL_MEMBER = 8520;

//
// MessageId: ERROR_DS_HAVE_PRIMARY_MEMBERS
//
// MessageText:
//
//  A group with primary members cannot change to a security-disabled group.
//
export const ERROR_DS_HAVE_PRIMARY_MEMBERS = 8521;

//
// MessageId: ERROR_DS_STRING_SD_CONVERSION_FAILED
//
// MessageText:
//
//  The schema cache load failed to convert the string default SD on a class-schema object.
//
export const ERROR_DS_STRING_SD_CONVERSION_FAILED = 8522;

//
// MessageId: ERROR_DS_NAMING_MASTER_GC
//
// MessageText:
//
//  Only DSAs configured to be Global Catalog servers should be allowed to hold the Domain Naming Master FSMO role. (Applies only to Windows 2000 servers)
//
export const ERROR_DS_NAMING_MASTER_GC = 8523;

//
// MessageId: ERROR_DS_DNS_LOOKUP_FAILURE
//
// MessageText:
//
//  The DSA operation is unable to proceed because of a DNS lookup failure.
//
export const ERROR_DS_DNS_LOOKUP_FAILURE = 8524;

//
// MessageId: ERROR_DS_COULDNT_UPDATE_SPNS
//
// MessageText:
//
//  While processing a change to the DNS Host Name for an object, the Service Principal Name values could not be kept in sync.
//
export const ERROR_DS_COULDNT_UPDATE_SPNS = 8525;

//
// MessageId: ERROR_DS_CANT_RETRIEVE_SD
//
// MessageText:
//
//  The Security Descriptor attribute could not be read.
//
export const ERROR_DS_CANT_RETRIEVE_SD = 8526;

//
// MessageId: ERROR_DS_KEY_NOT_UNIQUE
//
// MessageText:
//
//  The object requested was not found, but an object with that key was found.
//
export const ERROR_DS_KEY_NOT_UNIQUE = 8527;

//
// MessageId: ERROR_DS_WRONG_LINKED_ATT_SYNTAX
//
// MessageText:
//
//  The syntax of the linked attribute being added is incorrect. Forward links can only have syntax 2.5.5.1, 2.5.5.7, and 2.5.5.14, and backlinks can only have syntax 2.5.5.1
//
export const ERROR_DS_WRONG_LINKED_ATT_SYNTAX = 8528;

//
// MessageId: ERROR_DS_SAM_NEED_BOOTKEY_PASSWORD
//
// MessageText:
//
//  Security Account Manager needs to get the boot password.
//
export const ERROR_DS_SAM_NEED_BOOTKEY_PASSWORD = 8529;

//
// MessageId: ERROR_DS_SAM_NEED_BOOTKEY_FLOPPY
//
// MessageText:
//
//  Security Account Manager needs to get the boot key from floppy disk.
//
export const ERROR_DS_SAM_NEED_BOOTKEY_FLOPPY = 8530;

//
// MessageId: ERROR_DS_CANT_START
//
// MessageText:
//
//  Directory Service cannot start.
//
export const ERROR_DS_CANT_START = 8531;

//
// MessageId: ERROR_DS_INIT_FAILURE
//
// MessageText:
//
//  Directory Services could not start.
//
export const ERROR_DS_INIT_FAILURE = 8532;

//
// MessageId: ERROR_DS_NO_PKT_PRIVACY_ON_CONNECTION
//
// MessageText:
//
//  The connection between client and server requires packet privacy or better.
//
export const ERROR_DS_NO_PKT_PRIVACY_ON_CONNECTION = 8533;

//
// MessageId: ERROR_DS_SOURCE_DOMAIN_IN_FOREST
//
// MessageText:
//
//  The source domain may not be in the same forest as destination.
//
export const ERROR_DS_SOURCE_DOMAIN_IN_FOREST = 8534;

//
// MessageId: ERROR_DS_DESTINATION_DOMAIN_NOT_IN_FOREST
//
// MessageText:
//
//  The destination domain must be in the forest.
//
export const ERROR_DS_DESTINATION_DOMAIN_NOT_IN_FOREST = 8535;

//
// MessageId: ERROR_DS_DESTINATION_AUDITING_NOT_ENABLED
//
// MessageText:
//
//  The operation requires that destination domain auditing be enabled.
//
export const ERROR_DS_DESTINATION_AUDITING_NOT_ENABLED = 8536;

//
// MessageId: ERROR_DS_CANT_FIND_DC_FOR_SRC_DOMAIN
//
// MessageText:
//
//  The operation couldn't locate a DC for the source domain.
//
export const ERROR_DS_CANT_FIND_DC_FOR_SRC_DOMAIN = 8537;

//
// MessageId: ERROR_DS_SRC_OBJ_NOT_GROUP_OR_USER
//
// MessageText:
//
//  The source object must be a group or user.
//
export const ERROR_DS_SRC_OBJ_NOT_GROUP_OR_USER = 8538;

//
// MessageId: ERROR_DS_SRC_SID_EXISTS_IN_FOREST
//
// MessageText:
//
//  The source object's SID already exists in destination forest.
//
export const ERROR_DS_SRC_SID_EXISTS_IN_FOREST = 8539;

//
// MessageId: ERROR_DS_SRC_AND_DST_OBJECT_CLASS_MISMATCH
//
// MessageText:
//
//  The source and destination object must be of the same type.
//
export const ERROR_DS_SRC_AND_DST_OBJECT_CLASS_MISMATCH = 8540;

//
// MessageId: ERROR_SAM_INIT_FAILURE
//
// MessageText:
//
//  Security Accounts Manager initialization failed because of the following error: %1.
//  Error Status: 0x%2. Click OK to shut down the system and reboot into Safe Mode. Check the event log for detailed information.
//
export const ERROR_SAM_INIT_FAILURE = 8541;

//
// MessageId: ERROR_DS_DRA_SCHEMA_INFO_SHIP
//
// MessageText:
//
//  Schema information could not be included in the replication request.
//
export const ERROR_DS_DRA_SCHEMA_INFO_SHIP = 8542;

//
// MessageId: ERROR_DS_DRA_SCHEMA_CONFLICT
//
// MessageText:
//
//  The replication operation could not be completed due to a schema incompatibility.
//
export const ERROR_DS_DRA_SCHEMA_CONFLICT = 8543;

//
// MessageId: ERROR_DS_DRA_EARLIER_SCHEMA_CONFLICT
//
// MessageText:
//
//  The replication operation could not be completed due to a previous schema incompatibility.
//
export const ERROR_DS_DRA_EARLIER_SCHEMA_CONFLICT = 8544;

//
// MessageId: ERROR_DS_DRA_OBJ_NC_MISMATCH
//
// MessageText:
//
//  The replication update could not be applied because either the source or the destination has not yet received information regarding a recent cross-domain move operation.
//
export const ERROR_DS_DRA_OBJ_NC_MISMATCH = 8545;

//
// MessageId: ERROR_DS_NC_STILL_HAS_DSAS
//
// MessageText:
//
//  The requested domain could not be deleted because there exist domain controllers that still host this domain.
//
export const ERROR_DS_NC_STILL_HAS_DSAS = 8546;

//
// MessageId: ERROR_DS_GC_REQUIRED
//
// MessageText:
//
//  The requested operation can be performed only on a global catalog server.
//
export const ERROR_DS_GC_REQUIRED = 8547;

//
// MessageId: ERROR_DS_LOCAL_MEMBER_OF_LOCAL_ONLY
//
// MessageText:
//
//  A local group can only be a member of other local groups in the same domain.
//
export const ERROR_DS_LOCAL_MEMBER_OF_LOCAL_ONLY = 8548;

//
// MessageId: ERROR_DS_NO_FPO_IN_UNIVERSAL_GROUPS
//
// MessageText:
//
//  Foreign security principals cannot be members of universal groups.
//
export const ERROR_DS_NO_FPO_IN_UNIVERSAL_GROUPS = 8549;

//
// MessageId: ERROR_DS_CANT_ADD_TO_GC
//
// MessageText:
//
//  The attribute is not allowed to be replicated to the GC because of security reasons.
//
export const ERROR_DS_CANT_ADD_TO_GC = 8550;

//
// MessageId: ERROR_DS_NO_CHECKPOINT_WITH_PDC
//
// MessageText:
//
//  The checkpoint with the PDC could not be taken because there too many modifications being processed currently.
//
export const ERROR_DS_NO_CHECKPOINT_WITH_PDC = 8551;

//
// MessageId: ERROR_DS_SOURCE_AUDITING_NOT_ENABLED
//
// MessageText:
//
//  The operation requires that source domain auditing be enabled.
//
export const ERROR_DS_SOURCE_AUDITING_NOT_ENABLED = 8552;

//
// MessageId: ERROR_DS_CANT_CREATE_IN_NONDOMAIN_NC
//
// MessageText:
//
//  Security principal objects can only be created inside domain naming contexts.
//
export const ERROR_DS_CANT_CREATE_IN_NONDOMAIN_NC = 8553;

//
// MessageId: ERROR_DS_INVALID_NAME_FOR_SPN
//
// MessageText:
//
//  A Service Principal Name (SPN) could not be export constructed because the provided hostname is not in the necessary format.
//
export const ERROR_DS_INVALID_NAME_FOR_SPN = 8554;

//
// MessageId: ERROR_DS_FILTER_USES_CONTRUCTED_ATTRS
//
// MessageText:
//
//  A Filter was passed that uses export constructed attributes.
//
export const ERROR_DS_FILTER_USES_CONTRUCTED_ATTRS = 8555;

//
// MessageId: ERROR_DS_UNICODEPWD_NOT_IN_QUOTES
//
// MessageText:
//
//  The unicodePwd attribute value must be enclosed in double quotes.
//
export const ERROR_DS_UNICODEPWD_NOT_IN_QUOTES = 8556;

//
// MessageId: ERROR_DS_MACHINE_ACCOUNT_QUOTA_EXCEEDED
//
// MessageText:
//
//  Your computer could not be joined to the domain. You have exceeded the maximum number of computer accounts you are allowed to create in this domain. Contact your system administrator to have this limit reset or increased.
//
export const ERROR_DS_MACHINE_ACCOUNT_QUOTA_EXCEEDED = 8557;

//
// MessageId: ERROR_DS_MUST_BE_RUN_ON_DST_DC
//
// MessageText:
//
//  For security reasons, the operation must be run on the destination DC.
//
export const ERROR_DS_MUST_BE_RUN_ON_DST_DC = 8558;

//
// MessageId: ERROR_DS_SRC_DC_MUST_BE_SP4_OR_GREATER
//
// MessageText:
//
//  For security reasons, the source DC must be NT4SP4 or greater.
//
export const ERROR_DS_SRC_DC_MUST_BE_SP4_OR_GREATER = 8559;

//
// MessageId: ERROR_DS_CANT_TREE_DELETE_CRITICAL_OBJ
//
// MessageText:
//
//  Critical Directory Service System objects cannot be deleted during tree delete operations.  The tree delete may have been partially performed.
//
export const ERROR_DS_CANT_TREE_DELETE_CRITICAL_OBJ = 8560;

//
// MessageId: ERROR_DS_INIT_FAILURE_CONSOLE
//
// MessageText:
//
//  Directory Services could not start because of the following error: %1.
//  Error Status: 0x%2. Please click OK to shutdown the system. You can use the recovery console to diagnose the system further.
//
export const ERROR_DS_INIT_FAILURE_CONSOLE = 8561;

//
// MessageId: ERROR_DS_SAM_INIT_FAILURE_CONSOLE
//
// MessageText:
//
//  Security Accounts Manager initialization failed because of the following error: %1.
//  Error Status: 0x%2. Please click OK to shutdown the system. You can use the recovery console to diagnose the system further.
//
export const ERROR_DS_SAM_INIT_FAILURE_CONSOLE = 8562;

//
// MessageId: ERROR_DS_FOREST_VERSION_TOO_HIGH
//
// MessageText:
//
//  The version of the operating system installed is incompatible with the current forest functional level. You must upgrade to a new version of the operating system before this server can become a domain controller in this forest.
//
export const ERROR_DS_FOREST_VERSION_TOO_HIGH = 8563;

//
// MessageId: ERROR_DS_DOMAIN_VERSION_TOO_HIGH
//
// MessageText:
//
//  The version of the operating system installed is incompatible with the current domain functional level. You must upgrade to a new version of the operating system before this server can become a domain controller in this domain.
//
export const ERROR_DS_DOMAIN_VERSION_TOO_HIGH = 8564;

//
// MessageId: ERROR_DS_FOREST_VERSION_TOO_LOW
//
// MessageText:
//
//  The version of the operating system installed on this server no longer supports the current forest functional level. You must raise the forest functional level before this server can become a domain controller in this forest.
//
export const ERROR_DS_FOREST_VERSION_TOO_LOW = 8565;

//
// MessageId: ERROR_DS_DOMAIN_VERSION_TOO_LOW
//
// MessageText:
//
//  The version of the operating system installed on this server no longer supports the current domain functional level. You must raise the domain functional level before this server can become a domain controller in this domain.
//
export const ERROR_DS_DOMAIN_VERSION_TOO_LOW = 8566;

//
// MessageId: ERROR_DS_INCOMPATIBLE_VERSION
//
// MessageText:
//
//  The version of the operating system installed on this server is incompatible with the functional level of the domain or forest.
//
export const ERROR_DS_INCOMPATIBLE_VERSION = 8567;

//
// MessageId: ERROR_DS_LOW_DSA_VERSION
//
// MessageText:
//
//  The functional level of the domain (or forest) cannot be raised to the requested value, because there exist one or more domain controllers in the domain (or forest) that are at a lower incompatible functional level.
//
export const ERROR_DS_LOW_DSA_VERSION = 8568;

//
// MessageId: ERROR_DS_NO_BEHAVIOR_VERSION_IN_MIXEDDOMAIN
//
// MessageText:
//
//  The forest functional level cannot be raised to the requested value since one or more domains are still in mixed domain mode. All domains in the forest must be in native mode, for you to raise the forest functional level.
//
export const ERROR_DS_NO_BEHAVIOR_VERSION_IN_MIXEDDOMAIN = 8569;

//
// MessageId: ERROR_DS_NOT_SUPPORTED_SORT_ORDER
//
// MessageText:
//
//  The sort order requested is not supported.
//
export const ERROR_DS_NOT_SUPPORTED_SORT_ORDER = 8570;

//
// MessageId: ERROR_DS_NAME_NOT_UNIQUE
//
// MessageText:
//
//  The requested name already exists as a unique identifier.
//
export const ERROR_DS_NAME_NOT_UNIQUE = 8571;

//
// MessageId: ERROR_DS_MACHINE_ACCOUNT_CREATED_PRENT4
//
// MessageText:
//
//  The machine account was created pre-NT4.  The account needs to be recreated.
//
export const ERROR_DS_MACHINE_ACCOUNT_CREATED_PRENT4 = 8572;

//
// MessageId: ERROR_DS_OUT_OF_VERSION_STORE
//
// MessageText:
//
//  The database is out of version store.
//
export const ERROR_DS_OUT_OF_VERSION_STORE = 8573;

//
// MessageId: ERROR_DS_INCOMPATIBLE_CONTROLS_USED
//
// MessageText:
//
//  Unable to continue operation because multiple conflicting controls were used.
//
export const ERROR_DS_INCOMPATIBLE_CONTROLS_USED = 8574;

//
// MessageId: ERROR_DS_NO_REF_DOMAIN
//
// MessageText:
//
//  Unable to find a valid security descriptor reference domain for this partition.
//
export const ERROR_DS_NO_REF_DOMAIN = 8575;

//
// MessageId: ERROR_DS_RESERVED_LINK_ID
//
// MessageText:
//
//  Schema update failed: The link identifier is reserved.
//
export const ERROR_DS_RESERVED_LINK_ID = 8576;

//
// MessageId: ERROR_DS_LINK_ID_NOT_AVAILABLE
//
// MessageText:
//
//  Schema update failed: There are no link identifiers available.
//
export const ERROR_DS_LINK_ID_NOT_AVAILABLE = 8577;

//
// MessageId: ERROR_DS_AG_CANT_HAVE_UNIVERSAL_MEMBER
//
// MessageText:
//
//  An account group can not have a universal group as a member.
//
export const ERROR_DS_AG_CANT_HAVE_UNIVERSAL_MEMBER = 8578;

//
// MessageId: ERROR_DS_MODIFYDN_DISALLOWED_BY_INSTANCE_TYPE
//
// MessageText:
//
//  Rename or move operations on naming context heads or read-only objects are not allowed.
//
export const ERROR_DS_MODIFYDN_DISALLOWED_BY_INSTANCE_TYPE = 8579;

//
// MessageId: ERROR_DS_NO_OBJECT_MOVE_IN_SCHEMA_NC
//
// MessageText:
//
//  Move operations on objects in the schema naming context are not allowed.
//
export const ERROR_DS_NO_OBJECT_MOVE_IN_SCHEMA_NC = 8580;

//
// MessageId: ERROR_DS_MODIFYDN_DISALLOWED_BY_FLAG
//
// MessageText:
//
//  A system flag has been set on the object and does not allow the object to be moved or renamed.
//
export const ERROR_DS_MODIFYDN_DISALLOWED_BY_FLAG = 8581;

//
// MessageId: ERROR_DS_MODIFYDN_WRONG_GRANDPARENT
//
// MessageText:
//
//  This object is not allowed to change its grandparent container. Moves are not forbidden on this object, but are restricted to sibling containers.
//
export const ERROR_DS_MODIFYDN_WRONG_GRANDPARENT = 8582;

//
// MessageId: ERROR_DS_NAME_ERROR_TRUST_REFERRAL
//
// MessageText:
//
//  Unable to resolve completely, a referral to another forest is generated.
//
export const ERROR_DS_NAME_ERROR_TRUST_REFERRAL = 8583;

//
// MessageId: ERROR_NOT_SUPPORTED_ON_STANDARD_SERVER
//
// MessageText:
//
//  The requested action is not supported on standard server.
//
export const ERROR_NOT_SUPPORTED_ON_STANDARD_SERVER = 8584;

//
// MessageId: ERROR_DS_CANT_ACCESS_REMOTE_PART_OF_AD
//
// MessageText:
//
//  Could not access a partition of the Active Directory located on a remote server.  Make sure at least one server is running for the partition in question.
//
export const ERROR_DS_CANT_ACCESS_REMOTE_PART_OF_AD = 8585;

//
// MessageId: ERROR_DS_CR_IMPOSSIBLE_TO_VALIDATE_V2
//
// MessageText:
//
//  The directory cannot validate the proposed naming context (or partition) name because it does not hold a replica nor can it contact a replica of the naming context above the proposed naming context.  Please ensure that the parent naming context is properly registered in DNS, and at least one replica of this naming context is reachable by the Domain Naming master.
//
export const ERROR_DS_CR_IMPOSSIBLE_TO_VALIDATE_V2 = 8586;

//
// MessageId: ERROR_DS_THREAD_LIMIT_EXCEEDED
//
// MessageText:
//
//  The thread limit for this request was exceeded.
//
export const ERROR_DS_THREAD_LIMIT_EXCEEDED = 8587;

//
// MessageId: ERROR_DS_NOT_CLOSEST
//
// MessageText:
//
//  The Global catalog server is not in the closest site.
//
export const ERROR_DS_NOT_CLOSEST = 8588;

//
// MessageId: ERROR_DS_CANT_DERIVE_SPN_WITHOUT_SERVER_REF
//
// MessageText:
//
//  The DS cannot derive a service principal name (SPN) with which to mutually authenticate the target server because the corresponding server object in the local DS database has no serverReference attribute.
//
export const ERROR_DS_CANT_DERIVE_SPN_WITHOUT_SERVER_REF = 8589;

//
// MessageId: ERROR_DS_SINGLE_USER_MODE_FAILED
//
// MessageText:
//
//  The Directory Service failed to enter single user mode.
//
export const ERROR_DS_SINGLE_USER_MODE_FAILED = 8590;

//
// MessageId: ERROR_DS_NTDSCRIPT_SYNTAX_ERROR
//
// MessageText:
//
//  The Directory Service cannot parse the script because of a syntax error.
//
export const ERROR_DS_NTDSCRIPT_SYNTAX_ERROR = 8591;

//
// MessageId: ERROR_DS_NTDSCRIPT_PROCESS_ERROR
//
// MessageText:
//
//  The Directory Service cannot process the script because of an error.
//
export const ERROR_DS_NTDSCRIPT_PROCESS_ERROR = 8592;

//
// MessageId: ERROR_DS_DIFFERENT_REPL_EPOCHS
//
// MessageText:
//
//  The directory service cannot perform the requested operation because the servers
//  involved are of different replication epochs (which is usually related to a
//  domain rename that is in progress).
//
export const ERROR_DS_DIFFERENT_REPL_EPOCHS = 8593;

//
// MessageId: ERROR_DS_DRS_EXTENSIONS_CHANGED
//
// MessageText:
//
//  The directory service binding must be renegotiated due to a change in the server
//  extensions information.
//
export const ERROR_DS_DRS_EXTENSIONS_CHANGED = 8594;

//
// MessageId: ERROR_DS_REPLICA_SET_CHANGE_NOT_ALLOWED_ON_DISABLED_CR
//
// MessageText:
//
//  Operation not allowed on a disabled cross ref.
//
export const ERROR_DS_REPLICA_SET_CHANGE_NOT_ALLOWED_ON_DISABLED_CR = 8595;

//
// MessageId: ERROR_DS_NO_MSDS_INTID
//
// MessageText:
//
//  Schema update failed: No values for msDS-IntId are available.
//
export const ERROR_DS_NO_MSDS_INTID = 8596;

//
// MessageId: ERROR_DS_DUP_MSDS_INTID
//
// MessageText:
//
//  Schema update failed: Duplicate msDS-INtId. Retry the operation.
//
export const ERROR_DS_DUP_MSDS_INTID = 8597;

//
// MessageId: ERROR_DS_EXISTS_IN_RDNATTID
//
// MessageText:
//
//  Schema deletion failed: attribute is used in rDNAttID.
//
export const ERROR_DS_EXISTS_IN_RDNATTID = 8598;

//
// MessageId: ERROR_DS_AUTHORIZATION_FAILED
//
// MessageText:
//
//  The directory service failed to authorize the request.
//
export const ERROR_DS_AUTHORIZATION_FAILED = 8599;

//
// MessageId: ERROR_DS_INVALID_SCRIPT
//
// MessageText:
//
//  The Directory Service cannot process the script because it is invalid.
//
export const ERROR_DS_INVALID_SCRIPT = 8600;

//
// MessageId: ERROR_DS_REMOTE_CROSSREF_OP_FAILED
//
// MessageText:
//
//  The remote create cross reference operation failed on the Domain Naming Master FSMO.  The operation's error is in the extended data.
//
export const ERROR_DS_REMOTE_CROSSREF_OP_FAILED = 8601;

//
// MessageId: ERROR_DS_CROSS_REF_BUSY
//
// MessageText:
//
//  A cross reference is in use locally with the same name.
//
export const ERROR_DS_CROSS_REF_BUSY = 8602;

//
// MessageId: ERROR_DS_CANT_DERIVE_SPN_FOR_DELETED_DOMAIN
//
// MessageText:
//
//  The DS cannot derive a service principal name (SPN) with which to mutually authenticate the target server because the server's domain has been deleted from the forest.
//
export const ERROR_DS_CANT_DERIVE_SPN_FOR_DELETED_DOMAIN = 8603;

//
// MessageId: ERROR_DS_CANT_DEMOTE_WITH_WRITEABLE_NC
//
// MessageText:
//
//  Writeable NCs prevent this DC from demoting.
//
export const ERROR_DS_CANT_DEMOTE_WITH_WRITEABLE_NC = 8604;

//
// MessageId: ERROR_DS_DUPLICATE_ID_FOUND
//
// MessageText:
//
//  The requested object has a non-unique identifier and cannot be retrieved.
//
export const ERROR_DS_DUPLICATE_ID_FOUND = 8605;

//
// MessageId: ERROR_DS_INSUFFICIENT_ATTR_TO_CREATE_OBJECT
//
// MessageText:
//
//  Insufficient attributes were given to create an object.  This object may not exist because it may have been deleted and already garbage collected.
//
export const ERROR_DS_INSUFFICIENT_ATTR_TO_CREATE_OBJECT = 8606;

//
// MessageId: ERROR_DS_GROUP_CONVERSION_ERROR
//
// MessageText:
//
//  The group cannot be converted due to attribute restrictions on the requested group type.
//
export const ERROR_DS_GROUP_CONVERSION_ERROR = 8607;

//
// MessageId: ERROR_DS_CANT_MOVE_APP_BASIC_GROUP
//
// MessageText:
//
//  Cross-domain move of non-empty basic application groups is not allowed.
//
export const ERROR_DS_CANT_MOVE_APP_BASIC_GROUP = 8608;

//
// MessageId: ERROR_DS_CANT_MOVE_APP_QUERY_GROUP
//
// MessageText:
//
//  Cross-domain move of non-empty query based application groups is not allowed.
//
export const ERROR_DS_CANT_MOVE_APP_QUERY_GROUP = 8609;

//
// MessageId: ERROR_DS_ROLE_NOT_VERIFIED
//
// MessageText:
//
//  The FSMO role ownership could not be verified because its directory partition has not replicated successfully with atleast one replication partner.
//
export const ERROR_DS_ROLE_NOT_VERIFIED = 8610;

//
// MessageId: ERROR_DS_WKO_CONTAINER_CANNOT_BE_SPECIAL
//
// MessageText:
//
//  The target container for a redirection of a well known object container cannot already be a special container.
//
export const ERROR_DS_WKO_CONTAINER_CANNOT_BE_SPECIAL = 8611;

//
// MessageId: ERROR_DS_DOMAIN_RENAME_IN_PROGRESS
//
// MessageText:
//
//  The Directory Service cannot perform the requested operation because a domain rename operation is in progress.
//
export const ERROR_DS_DOMAIN_RENAME_IN_PROGRESS = 8612;

//
// MessageId: ERROR_DS_EXISTING_AD_CHILD_NC
//
// MessageText:
//
//  The Active Directory detected an Active Directory child partition below the
//  requested new partition name.  The Active Directory's partition hierarchy must
//  be created in a top down method.
//
export const ERROR_DS_EXISTING_AD_CHILD_NC = 8613;

//
// MessageId: ERROR_DS_REPL_LIFETIME_EXCEEDED
//
// MessageText:
//
//  The Active Directory cannot replicate with this server because the time since the last replication with this server has exceeded the tombstone lifetime.
//
export const ERROR_DS_REPL_LIFETIME_EXCEEDED = 8614;

//
// MessageId: ERROR_DS_DISALLOWED_IN_SYSTEM_CONTAINER
//
// MessageText:
//
//  The requested operation is not allowed on an object under the system container.
//
export const ERROR_DS_DISALLOWED_IN_SYSTEM_CONTAINER = 8615;

//
// MessageId: ERROR_DS_LDAP_SEND_QUEUE_FULL
//
// MessageText:
//
//  The LDAP servers network send queue has filled up because the client is not
//  processing the results of it's requests fast enough.  No more requests will
//  be processed until the client catches up.  If the client does not catch up
//  then it will be disconnected.
//
export const ERROR_DS_LDAP_SEND_QUEUE_FULL = 8616;

//
// MessageId: ERROR_DS_DRA_OUT_SCHEDULE_WINDOW
//
// MessageText:
//
//  The scheduled replication did not take place because the system was too busy to execute the request within the schedule window.  The replication queue is overloaded. Consider reducing the number of partners or decreasing the scheduled replication frequency.
//
export const ERROR_DS_DRA_OUT_SCHEDULE_WINDOW = 8617;

///////////////////////////////////////////////////
//                                                /
//     End of Active Directory Error Codes        /
//                                                /
//                  8000 to  8999                 /
///////////////////////////////////////////////////

///////////////////////////////////////////////////
//                                               //
//                  DNS Error Codes              //
//                                               //
//                   9000 to 9999                //
///////////////////////////////////////////////////

// =============================
// Facility DNS Error Messages
// =============================

//
//  DNS response codes.
//

export const DNS_ERROR_RESPONSE_CODES_BASE = 9000;

// DNS_ERROR_RCODE_FORMAT_ERROR          0x00002329
//
// MessageId: DNS_ERROR_RCODE_FORMAT_ERROR
//
// MessageText:
//
//  DNS server unable to interpret format.
//
export const DNS_ERROR_RCODE_FORMAT_ERROR = 9001;

// DNS_ERROR_RCODE_SERVER_FAILURE        0x0000232a
//
// MessageId: DNS_ERROR_RCODE_SERVER_FAILURE
//
// MessageText:
//
//  DNS server failure.
//
export const DNS_ERROR_RCODE_SERVER_FAILURE = 9002;

// DNS_ERROR_RCODE_NAME_ERROR            0x0000232b
//
// MessageId: DNS_ERROR_RCODE_NAME_ERROR
//
// MessageText:
//
//  DNS name does not exist.
//
export const DNS_ERROR_RCODE_NAME_ERROR = 9003;

// DNS_ERROR_RCODE_NOT_IMPLEMENTED       0x0000232c
//
// MessageId: DNS_ERROR_RCODE_NOT_IMPLEMENTED
//
// MessageText:
//
//  DNS request not supported by name server.
//
export const DNS_ERROR_RCODE_NOT_IMPLEMENTED = 9004;

// DNS_ERROR_RCODE_REFUSED               0x0000232d
//
// MessageId: DNS_ERROR_RCODE_REFUSED
//
// MessageText:
//
//  DNS operation refused.
//
export const DNS_ERROR_RCODE_REFUSED = 9005;

// DNS_ERROR_RCODE_YXDOMAIN              0x0000232e
//
// MessageId: DNS_ERROR_RCODE_YXDOMAIN
//
// MessageText:
//
//  DNS name that ought not exist, does exist.
//
export const DNS_ERROR_RCODE_YXDOMAIN = 9006;

// DNS_ERROR_RCODE_YXRRSET               0x0000232f
//
// MessageId: DNS_ERROR_RCODE_YXRRSET
//
// MessageText:
//
//  DNS RR set that ought not exist, does exist.
//
export const DNS_ERROR_RCODE_YXRRSET = 9007;

// DNS_ERROR_RCODE_NXRRSET               0x00002330
//
// MessageId: DNS_ERROR_RCODE_NXRRSET
//
// MessageText:
//
//  DNS RR set that ought to exist, does not exist.
//
export const DNS_ERROR_RCODE_NXRRSET = 9008;

// DNS_ERROR_RCODE_NOTAUTH               0x00002331
//
// MessageId: DNS_ERROR_RCODE_NOTAUTH
//
// MessageText:
//
//  DNS server not authoritative for zone.
//
export const DNS_ERROR_RCODE_NOTAUTH = 9009;

// DNS_ERROR_RCODE_NOTZONE               0x00002332
//
// MessageId: DNS_ERROR_RCODE_NOTZONE
//
// MessageText:
//
//  DNS name in update or prereq is not in zone.
//
export const DNS_ERROR_RCODE_NOTZONE = 9010;

// DNS_ERROR_RCODE_BADSIG                0x00002338
//
// MessageId: DNS_ERROR_RCODE_BADSIG
//
// MessageText:
//
//  DNS signature failed to verify.
//
export const DNS_ERROR_RCODE_BADSIG = 9016;

// DNS_ERROR_RCODE_BADKEY                0x00002339
//
// MessageId: DNS_ERROR_RCODE_BADKEY
//
// MessageText:
//
//  DNS bad key.
//
export const DNS_ERROR_RCODE_BADKEY = 9017;

// DNS_ERROR_RCODE_BADTIME               0x0000233a
//
// MessageId: DNS_ERROR_RCODE_BADTIME
//
// MessageText:
//
//  DNS signature validity expired.
//
export const DNS_ERROR_RCODE_BADTIME = 9018;

//
//  Packet format
//

export const DNS_ERROR_PACKET_FMT_BASE = 9500;

// DNS_INFO_NO_RECORDS                   0x0000251d
//
// MessageId: DNS_INFO_NO_RECORDS
//
// MessageText:
//
//  No records found for given DNS query.
//
export const DNS_INFO_NO_RECORDS = 9501;

// DNS_ERROR_BAD_PACKET                  0x0000251e
//
// MessageId: DNS_ERROR_BAD_PACKET
//
// MessageText:
//
//  Bad DNS packet.
//
export const DNS_ERROR_BAD_PACKET = 9502;

// DNS_ERROR_NO_PACKET                   0x0000251f
//
// MessageId: DNS_ERROR_NO_PACKET
//
// MessageText:
//
//  No DNS packet.
//
export const DNS_ERROR_NO_PACKET = 9503;

// DNS_ERROR_RCODE                       0x00002520
//
// MessageId: DNS_ERROR_RCODE
//
// MessageText:
//
//  DNS error, check rcode.
//
export const DNS_ERROR_RCODE = 9504;

// DNS_ERROR_UNSECURE_PACKET             0x00002521
//
// MessageId: DNS_ERROR_UNSECURE_PACKET
//
// MessageText:
//
//  Unsecured DNS packet.
//
export const DNS_ERROR_UNSECURE_PACKET = 9505;

//
//  General API errors
//

// DNS_ERROR_INVALID_TYPE                0x0000254f
//
// MessageId: DNS_ERROR_INVALID_TYPE
//
// MessageText:
//
//  Invalid DNS type.
//
export const DNS_ERROR_INVALID_TYPE = 9551;

// DNS_ERROR_INVALID_IP_ADDRESS          0x00002550
//
// MessageId: DNS_ERROR_INVALID_IP_ADDRESS
//
// MessageText:
//
//  Invalid IP address.
//
export const DNS_ERROR_INVALID_IP_ADDRESS = 9552;

// DNS_ERROR_INVALID_PROPERTY            0x00002551
//
// MessageId: DNS_ERROR_INVALID_PROPERTY
//
// MessageText:
//
//  Invalid property.
//
export const DNS_ERROR_INVALID_PROPERTY = 9553;

// DNS_ERROR_TRY_AGAIN_LATER             0x00002552
//
// MessageId: DNS_ERROR_TRY_AGAIN_LATER
//
// MessageText:
//
//  Try DNS operation again later.
//
export const DNS_ERROR_TRY_AGAIN_LATER = 9554;

// DNS_ERROR_NOT_UNIQUE                  0x00002553
//
// MessageId: DNS_ERROR_NOT_UNIQUE
//
// MessageText:
//
//  Record for given name and type is not unique.
//
export const DNS_ERROR_NOT_UNIQUE = 9555;

// DNS_ERROR_NON_RFC_NAME                0x00002554
//
// MessageId: DNS_ERROR_NON_RFC_NAME
//
// MessageText:
//
//  DNS name does not comply with RFC specifications.
//
export const DNS_ERROR_NON_RFC_NAME = 9556;

// DNS_STATUS_FQDN                       0x00002555
//
// MessageId: DNS_STATUS_FQDN
//
// MessageText:
//
//  DNS name is a fully-qualified DNS name.
//
export const DNS_STATUS_FQDN = 9557;

// DNS_STATUS_DOTTED_NAME                0x00002556
//
// MessageId: DNS_STATUS_DOTTED_NAME
//
// MessageText:
//
//  DNS name is dotted (multi-label).
//
export const DNS_STATUS_DOTTED_NAME = 9558;

// DNS_STATUS_SINGLE_PART_NAME           0x00002557
//
// MessageId: DNS_STATUS_SINGLE_PART_NAME
//
// MessageText:
//
//  DNS name is a single-part name.
//
export const DNS_STATUS_SINGLE_PART_NAME = 9559;

// DNS_ERROR_INVALID_NAME_CHAR           0x00002558
//
// MessageId: DNS_ERROR_INVALID_NAME_CHAR
//
// MessageText:
//
//  DNS name contains an invalid character.
//
export const DNS_ERROR_INVALID_NAME_CHAR = 9560;

// DNS_ERROR_NUMERIC_NAME                0x00002559
//
// MessageId: DNS_ERROR_NUMERIC_NAME
//
// MessageText:
//
//  DNS name is entirely numeric.
//
export const DNS_ERROR_NUMERIC_NAME = 9561;

// DNS_ERROR_NOT_ALLOWED_ON_ROOT_SERVER  0x0000255A
//
// MessageId: DNS_ERROR_NOT_ALLOWED_ON_ROOT_SERVER
//
// MessageText:
//
//  The operation requested is not permitted on a DNS root server.
//
export const DNS_ERROR_NOT_ALLOWED_ON_ROOT_SERVER = 9562;

// DNS_ERROR_NOT_ALLOWED_UNDER_DELEGATION  0x0000255B
//
// MessageId: DNS_ERROR_NOT_ALLOWED_UNDER_DELEGATION
//
// MessageText:
//
//  The record could not be created because this part of the DNS namespace has
//  been delegated to another server.
//
export const DNS_ERROR_NOT_ALLOWED_UNDER_DELEGATION = 9563;

// DNS_ERROR_CANNOT_FIND_ROOT_HINTS  0x0000255C
//
// MessageId: DNS_ERROR_CANNOT_FIND_ROOT_HINTS
//
// MessageText:
//
//  The DNS server could not find a set of root hints.
//
export const DNS_ERROR_CANNOT_FIND_ROOT_HINTS = 9564;

// DNS_ERROR_INCONSISTENT_ROOT_HINTS  0x0000255D
//
// MessageId: DNS_ERROR_INCONSISTENT_ROOT_HINTS
//
// MessageText:
//
//  The DNS server found root hints but they were not consistent across
//  all adapters.
//
export const DNS_ERROR_INCONSISTENT_ROOT_HINTS = 9565;

//
//  Zone errors
//

export const DNS_ERROR_ZONE_BASE = 9600;

// DNS_ERROR_ZONE_DOES_NOT_EXIST         0x00002581
//
// MessageId: DNS_ERROR_ZONE_DOES_NOT_EXIST
//
// MessageText:
//
//  DNS zone does not exist.
//
export const DNS_ERROR_ZONE_DOES_NOT_EXIST = 9601;

// DNS_ERROR_NO_ZONE_INFO                0x00002582
//
// MessageId: DNS_ERROR_NO_ZONE_INFO
//
// MessageText:
//
//  DNS zone information not available.
//
export const DNS_ERROR_NO_ZONE_INFO = 9602;

// DNS_ERROR_INVALID_ZONE_OPERATION      0x00002583
//
// MessageId: DNS_ERROR_INVALID_ZONE_OPERATION
//
// MessageText:
//
//  Invalid operation for DNS zone.
//
export const DNS_ERROR_INVALID_ZONE_OPERATION = 9603;

// DNS_ERROR_ZONE_CONFIGURATION_ERROR    0x00002584
//
// MessageId: DNS_ERROR_ZONE_CONFIGURATION_ERROR
//
// MessageText:
//
//  Invalid DNS zone configuration.
//
export const DNS_ERROR_ZONE_CONFIGURATION_ERROR = 9604;

// DNS_ERROR_ZONE_HAS_NO_SOA_RECORD      0x00002585
//
// MessageId: DNS_ERROR_ZONE_HAS_NO_SOA_RECORD
//
// MessageText:
//
//  DNS zone has no start of authority (SOA) record.
//
export const DNS_ERROR_ZONE_HAS_NO_SOA_RECORD = 9605;

// DNS_ERROR_ZONE_HAS_NO_NS_RECORDS      0x00002586
//
// MessageId: DNS_ERROR_ZONE_HAS_NO_NS_RECORDS
//
// MessageText:
//
//  DNS zone has no Name Server (NS) record.
//
export const DNS_ERROR_ZONE_HAS_NO_NS_RECORDS = 9606;

// DNS_ERROR_ZONE_LOCKED                 0x00002587
//
// MessageId: DNS_ERROR_ZONE_LOCKED
//
// MessageText:
//
//  DNS zone is locked.
//
export const DNS_ERROR_ZONE_LOCKED = 9607;

// DNS_ERROR_ZONE_CREATION_FAILED        0x00002588
//
// MessageId: DNS_ERROR_ZONE_CREATION_FAILED
//
// MessageText:
//
//  DNS zone creation failed.
//
export const DNS_ERROR_ZONE_CREATION_FAILED = 9608;

// DNS_ERROR_ZONE_ALREADY_EXISTS         0x00002589
//
// MessageId: DNS_ERROR_ZONE_ALREADY_EXISTS
//
// MessageText:
//
//  DNS zone already exists.
//
export const DNS_ERROR_ZONE_ALREADY_EXISTS = 9609;

// DNS_ERROR_AUTOZONE_ALREADY_EXISTS     0x0000258a
//
// MessageId: DNS_ERROR_AUTOZONE_ALREADY_EXISTS
//
// MessageText:
//
//  DNS automatic zone already exists.
//
export const DNS_ERROR_AUTOZONE_ALREADY_EXISTS = 9610;

// DNS_ERROR_INVALID_ZONE_TYPE           0x0000258b
//
// MessageId: DNS_ERROR_INVALID_ZONE_TYPE
//
// MessageText:
//
//  Invalid DNS zone type.
//
export const DNS_ERROR_INVALID_ZONE_TYPE = 9611;

// DNS_ERROR_SECONDARY_REQUIRES_MASTER_IP 0x0000258c
//
// MessageId: DNS_ERROR_SECONDARY_REQUIRES_MASTER_IP
//
// MessageText:
//
//  Secondary DNS zone requires master IP address.
//
export const DNS_ERROR_SECONDARY_REQUIRES_MASTER_IP = 9612;

// DNS_ERROR_ZONE_NOT_SECONDARY          0x0000258d
//
// MessageId: DNS_ERROR_ZONE_NOT_SECONDARY
//
// MessageText:
//
//  DNS zone not secondary.
//
export const DNS_ERROR_ZONE_NOT_SECONDARY = 9613;

// DNS_ERROR_NEED_SECONDARY_ADDRESSES    0x0000258e
//
// MessageId: DNS_ERROR_NEED_SECONDARY_ADDRESSES
//
// MessageText:
//
//  Need secondary IP address.
//
export const DNS_ERROR_NEED_SECONDARY_ADDRESSES = 9614;

// DNS_ERROR_WINS_INIT_FAILED            0x0000258f
//
// MessageId: DNS_ERROR_WINS_INIT_FAILED
//
// MessageText:
//
//  WINS initialization failed.
//
export const DNS_ERROR_WINS_INIT_FAILED = 9615;

// DNS_ERROR_NEED_WINS_SERVERS           0x00002590
//
// MessageId: DNS_ERROR_NEED_WINS_SERVERS
//
// MessageText:
//
//  Need WINS servers.
//
export const DNS_ERROR_NEED_WINS_SERVERS = 9616;

// DNS_ERROR_NBSTAT_INIT_FAILED          0x00002591
//
// MessageId: DNS_ERROR_NBSTAT_INIT_FAILED
//
// MessageText:
//
//  NBTSTAT initialization call failed.
//
export const DNS_ERROR_NBSTAT_INIT_FAILED = 9617;

// DNS_ERROR_SOA_DELETE_INVALID          0x00002592
//
// MessageId: DNS_ERROR_SOA_DELETE_INVALID
//
// MessageText:
//
//  Invalid delete of start of authority (SOA)
//
export const DNS_ERROR_SOA_DELETE_INVALID = 9618;

// DNS_ERROR_FORWARDER_ALREADY_EXISTS    0x00002593
//
// MessageId: DNS_ERROR_FORWARDER_ALREADY_EXISTS
//
// MessageText:
//
//  A conditional forwarding zone already exists for that name.
//
export const DNS_ERROR_FORWARDER_ALREADY_EXISTS = 9619;

// DNS_ERROR_ZONE_REQUIRES_MASTER_IP     0x00002594
//
// MessageId: DNS_ERROR_ZONE_REQUIRES_MASTER_IP
//
// MessageText:
//
//  This zone must be configured with one or more master DNS server IP addresses.
//
export const DNS_ERROR_ZONE_REQUIRES_MASTER_IP = 9620;

// DNS_ERROR_ZONE_IS_SHUTDOWN            0x00002595
//
// MessageId: DNS_ERROR_ZONE_IS_SHUTDOWN
//
// MessageText:
//
//  The operation cannot be performed because this zone is shutdown.
//
export const DNS_ERROR_ZONE_IS_SHUTDOWN = 9621;

//
//  Datafile errors
//

// DNS                                   0x000025b3
//
// MessageId: DNS_ERROR_PRIMARY_REQUIRES_DATAFILE
//
// MessageText:
//
//  Primary DNS zone requires datafile.
//
export const DNS_ERROR_PRIMARY_REQUIRES_DATAFILE = 9651;

// DNS                                   0x000025b4
//
// MessageId: DNS_ERROR_INVALID_DATAFILE_NAME
//
// MessageText:
//
//  Invalid datafile name for DNS zone.
//
export const DNS_ERROR_INVALID_DATAFILE_NAME = 9652;

// DNS                                   0x000025b5
//
// MessageId: DNS_ERROR_DATAFILE_OPEN_FAILURE
//
// MessageText:
//
//  Failed to open datafile for DNS zone.
//
export const DNS_ERROR_DATAFILE_OPEN_FAILURE = 9653;

// DNS                                   0x000025b6
//
// MessageId: DNS_ERROR_FILE_WRITEBACK_FAILED
//
// MessageText:
//
//  Failed to write datafile for DNS zone.
//
export const DNS_ERROR_FILE_WRITEBACK_FAILED = 9654;

// DNS                                   0x000025b7
//
// MessageId: DNS_ERROR_DATAFILE_PARSING
//
// MessageText:
//
//  Failure while reading datafile for DNS zone.
//
export const DNS_ERROR_DATAFILE_PARSING = 9655;

//
//  Database errors
//

export const DNS_ERROR_DATABASE_BASE = 9700;

// DNS_ERROR_RECORD_DOES_NOT_EXIST       0x000025e5
//
// MessageId: DNS_ERROR_RECORD_DOES_NOT_EXIST
//
// MessageText:
//
//  DNS record does not exist.
//
export const DNS_ERROR_RECORD_DOES_NOT_EXIST = 9701;

// DNS_ERROR_RECORD_FORMAT               0x000025e6
//
// MessageId: DNS_ERROR_RECORD_FORMAT
//
// MessageText:
//
//  DNS record format error.
//
export const DNS_ERROR_RECORD_FORMAT = 9702;

// DNS_ERROR_NODE_CREATION_FAILED        0x000025e7
//
// MessageId: DNS_ERROR_NODE_CREATION_FAILED
//
// MessageText:
//
//  Node creation failure in DNS.
//
export const DNS_ERROR_NODE_CREATION_FAILED = 9703;

// DNS_ERROR_UNKNOWN_RECORD_TYPE         0x000025e8
//
// MessageId: DNS_ERROR_UNKNOWN_RECORD_TYPE
//
// MessageText:
//
//  Unknown DNS record type.
//
export const DNS_ERROR_UNKNOWN_RECORD_TYPE = 9704;

// DNS_ERROR_RECORD_TIMED_OUT            0x000025e9
//
// MessageId: DNS_ERROR_RECORD_TIMED_OUT
//
// MessageText:
//
//  DNS record timed out.
//
export const DNS_ERROR_RECORD_TIMED_OUT = 9705;

// DNS_ERROR_NAME_NOT_IN_ZONE            0x000025ea
//
// MessageId: DNS_ERROR_NAME_NOT_IN_ZONE
//
// MessageText:
//
//  Name not in DNS zone.
//
export const DNS_ERROR_NAME_NOT_IN_ZONE = 9706;

// DNS_ERROR_CNAME_LOOP                  0x000025eb
//
// MessageId: DNS_ERROR_CNAME_LOOP
//
// MessageText:
//
//  CNAME loop detected.
//
export const DNS_ERROR_CNAME_LOOP = 9707;

// DNS_ERROR_NODE_IS_CNAME               0x000025ec
//
// MessageId: DNS_ERROR_NODE_IS_CNAME
//
// MessageText:
//
//  Node is a CNAME DNS record.
//
export const DNS_ERROR_NODE_IS_CNAME = 9708;

// DNS_ERROR_CNAME_COLLISION             0x000025ed
//
// MessageId: DNS_ERROR_CNAME_COLLISION
//
// MessageText:
//
//  A CNAME record already exists for given name.
//
export const DNS_ERROR_CNAME_COLLISION = 9709;

// DNS_ERROR_RECORD_ONLY_AT_ZONE_ROOT    0x000025ee
//
// MessageId: DNS_ERROR_RECORD_ONLY_AT_ZONE_ROOT
//
// MessageText:
//
//  Record only at DNS zone root.
//
export const DNS_ERROR_RECORD_ONLY_AT_ZONE_ROOT = 9710;

// DNS_ERROR_RECORD_ALREADY_EXISTS       0x000025ef
//
// MessageId: DNS_ERROR_RECORD_ALREADY_EXISTS
//
// MessageText:
//
//  DNS record already exists.
//
export const DNS_ERROR_RECORD_ALREADY_EXISTS = 9711;

// DNS_ERROR_SECONDARY_DATA              0x000025f0
//
// MessageId: DNS_ERROR_SECONDARY_DATA
//
// MessageText:
//
//  Secondary DNS zone data error.
//
export const DNS_ERROR_SECONDARY_DATA = 9712;

// DNS_ERROR_NO_CREATE_CACHE_DATA        0x000025f1
//
// MessageId: DNS_ERROR_NO_CREATE_CACHE_DATA
//
// MessageText:
//
//  Could not create DNS cache data.
//
export const DNS_ERROR_NO_CREATE_CACHE_DATA = 9713;

// DNS_ERROR_NAME_DOES_NOT_EXIST         0x000025f2
//
// MessageId: DNS_ERROR_NAME_DOES_NOT_EXIST
//
// MessageText:
//
//  DNS name does not exist.
//
export const DNS_ERROR_NAME_DOES_NOT_EXIST = 9714;

// DNS_WARNING_PTR_CREATE_FAILED         0x000025f3
//
// MessageId: DNS_WARNING_PTR_CREATE_FAILED
//
// MessageText:
//
//  Could not create pointer (PTR) record.
//
export const DNS_WARNING_PTR_CREATE_FAILED = 9715;

// DNS_WARNING_DOMAIN_UNDELETED          0x000025f4
//
// MessageId: DNS_WARNING_DOMAIN_UNDELETED
//
// MessageText:
//
//  DNS domain was undeleted.
//
export const DNS_WARNING_DOMAIN_UNDELETED = 9716;

// DNS_ERROR_DS_UNAVAILABLE              0x000025f5
//
// MessageId: DNS_ERROR_DS_UNAVAILABLE
//
// MessageText:
//
//  The directory service is unavailable.
//
export const DNS_ERROR_DS_UNAVAILABLE = 9717;

// DNS_ERROR_DS_ZONE_ALREADY_EXISTS      0x000025f6
//
// MessageId: DNS_ERROR_DS_ZONE_ALREADY_EXISTS
//
// MessageText:
//
//  DNS zone already exists in the directory service.
//
export const DNS_ERROR_DS_ZONE_ALREADY_EXISTS = 9718;

// DNS_ERROR_NO_BOOTFILE_IF_DS_ZONE      0x000025f7
//
// MessageId: DNS_ERROR_NO_BOOTFILE_IF_DS_ZONE
//
// MessageText:
//
//  DNS server not creating or reading the boot file for the directory service integrated DNS zone.
//
export const DNS_ERROR_NO_BOOTFILE_IF_DS_ZONE = 9719;

//
//  Operation errors
//

export const DNS_ERROR_OPERATION_BASE = 9750;

// DNS_INFO_AXFR_COMPLETE                0x00002617
//
// MessageId: DNS_INFO_AXFR_COMPLETE
//
// MessageText:
//
//  DNS AXFR (zone transfer) complete.
//
export const DNS_INFO_AXFR_COMPLETE = 9751;

// DNS_ERROR_AXFR                        0x00002618
//
// MessageId: DNS_ERROR_AXFR
//
// MessageText:
//
//  DNS zone transfer failed.
//
export const DNS_ERROR_AXFR = 9752;

// DNS_INFO_ADDED_LOCAL_WINS             0x00002619
//
// MessageId: DNS_INFO_ADDED_LOCAL_WINS
//
// MessageText:
//
//  Added local WINS server.
//
export const DNS_INFO_ADDED_LOCAL_WINS = 9753;

//
//  Secure update
//

export const DNS_ERROR_SECURE_BASE = 9800;

// DNS_STATUS_CONTINUE_NEEDED            0x00002649
//
// MessageId: DNS_STATUS_CONTINUE_NEEDED
//
// MessageText:
//
//  Secure update call needs to continue update request.
//
export const DNS_STATUS_CONTINUE_NEEDED = 9801;

//
//  Setup errors
//

export const DNS_ERROR_SETUP_BASE = 9850;

// DNS_ERROR_NO_TCPIP                    0x0000267b
//
// MessageId: DNS_ERROR_NO_TCPIP
//
// MessageText:
//
//  TCP/IP network protocol not installed.
//
export const DNS_ERROR_NO_TCPIP = 9851;

// DNS_ERROR_NO_DNS_SERVERS              0x0000267c
//
// MessageId: DNS_ERROR_NO_DNS_SERVERS
//
// MessageText:
//
//  No DNS servers configured for local system.
//
export const DNS_ERROR_NO_DNS_SERVERS = 9852;

//
//  Directory partition (DP) errors
//

export const DNS_ERROR_DP_BASE = 9900;

// DNS_ERROR_DP_DOES_NOT_EXIST           0x000026ad
//
// MessageId: DNS_ERROR_DP_DOES_NOT_EXIST
//
// MessageText:
//
//  The specified directory partition does not exist.
//
export const DNS_ERROR_DP_DOES_NOT_EXIST = 9901;

// DNS_ERROR_DP_ALREADY_EXISTS           0x000026ae
//
// MessageId: DNS_ERROR_DP_ALREADY_EXISTS
//
// MessageText:
//
//  The specified directory partition already exists.
//
export const DNS_ERROR_DP_ALREADY_EXISTS = 9902;

// DNS_ERROR_DP_NOT_ENLISTED             0x000026af
//
// MessageId: DNS_ERROR_DP_NOT_ENLISTED
//
// MessageText:
//
//  This DNS server is not enlisted in the specified directory partition.
//
export const DNS_ERROR_DP_NOT_ENLISTED = 9903;

// DNS_ERROR_DP_ALREADY_ENLISTED         0x000026b0
//
// MessageId: DNS_ERROR_DP_ALREADY_ENLISTED
//
// MessageText:
//
//  This DNS server is already enlisted in the specified directory partition.
//
export const DNS_ERROR_DP_ALREADY_ENLISTED = 9904;

// DNS_ERROR_DP_NOT_AVAILABLE            0x000026b1
//
// MessageId: DNS_ERROR_DP_NOT_AVAILABLE
//
// MessageText:
//
//  The directory partition is not available at this time. Please wait
//  a few minutes and try again.
//
export const DNS_ERROR_DP_NOT_AVAILABLE = 9905;

// DNS_ERROR_DP_FSMO_ERROR               0x000026b2
//
// MessageId: DNS_ERROR_DP_FSMO_ERROR
//
// MessageText:
//
//  The application directory partition operation failed. The domain controller
//  holding the domain naming master role is down or unable to service the
//  request or is not running Windows Server 2003.
//
export const DNS_ERROR_DP_FSMO_ERROR = 9906;

///////////////////////////////////////////////////
//                                               //
//             End of DNS Error Codes            //
//                                               //
//                  9000 to 9999                 //
///////////////////////////////////////////////////

///////////////////////////////////////////////////
//                                               //
//               WinSock Error Codes             //
//                                               //
//                 10000 to 11999                //
///////////////////////////////////////////////////

//
// WinSock error codes are also defined in WinSock.h
// and WinSock2.h, hence the IFDEF
//
export const WSABASEERR = 10000;
//
// MessageId: WSAEINTR
//
// MessageText:
//
//  A blocking operation was interrupted by a call to WSACancelBlockingCall.
//
export const WSAEINTR = 10004;

//
// MessageId: WSAEBADF
//
// MessageText:
//
//  The file handle supplied is not valid.
//
export const WSAEBADF = 10009;

//
// MessageId: WSAEACCES
//
// MessageText:
//
//  An attempt was made to access a socket in a way forbidden by its access permissions.
//
export const WSAEACCES = 10013;

//
// MessageId: WSAEFAULT
//
// MessageText:
//
//  The system detected an invalid pointer address in attempting to use a pointer argument in a call.
//
export const WSAEFAULT = 10014;

//
// MessageId: WSAEINVAL
//
// MessageText:
//
//  An invalid argument was supplied.
//
export const WSAEINVAL = 10022;

//
// MessageId: WSAEMFILE
//
// MessageText:
//
//  Too many open sockets.
//
export const WSAEMFILE = 10024;

//
// MessageId: WSAEWOULDBLOCK
//
// MessageText:
//
//  A non-blocking socket operation could not be completed immediately.
//
export const WSAEWOULDBLOCK = 10035;

//
// MessageId: WSAEINPROGRESS
//
// MessageText:
//
//  A blocking operation is currently executing.
//
export const WSAEINPROGRESS = 10036;

//
// MessageId: WSAEALREADY
//
// MessageText:
//
//  An operation was attempted on a non-blocking socket that already had an operation in progress.
//
export const WSAEALREADY = 10037;

//
// MessageId: WSAENOTSOCK
//
// MessageText:
//
//  An operation was attempted on something that is not a socket.
//
export const WSAENOTSOCK = 10038;

//
// MessageId: WSAEDESTADDRREQ
//
// MessageText:
//
//  A required address was omitted from an operation on a socket.
//
export const WSAEDESTADDRREQ = 10039;

//
// MessageId: WSAEMSGSIZE
//
// MessageText:
//
//  A message sent on a datagram socket was larger than the internal message buffer or some other network limit, or the buffer used to receive a datagram into was smaller than the datagram itself.
//
export const WSAEMSGSIZE = 10040;

//
// MessageId: WSAEPROTOTYPE
//
// MessageText:
//
//  A protocol was specified in the socket function call that does not support the semantics of the socket type requested.
//
export const WSAEPROTOTYPE = 10041;

//
// MessageId: WSAENOPROTOOPT
//
// MessageText:
//
//  An unknown, invalid, or unsupported option or level was specified in a getsockopt or setsockopt call.
//
export const WSAENOPROTOOPT = 10042;

//
// MessageId: WSAEPROTONOSUPPORT
//
// MessageText:
//
//  The requested protocol has not been configured into the system, or no implementation for it exists.
//
export const WSAEPROTONOSUPPORT = 10043;

//
// MessageId: WSAESOCKTNOSUPPORT
//
// MessageText:
//
//  The support for the specified socket type does not exist in this address family.
//
export const WSAESOCKTNOSUPPORT = 10044;

//
// MessageId: WSAEOPNOTSUPP
//
// MessageText:
//
//  The attempted operation is not supported for the type of object referenced.
//
export const WSAEOPNOTSUPP = 10045;

//
// MessageId: WSAEPFNOSUPPORT
//
// MessageText:
//
//  The protocol family has not been configured into the system or no implementation for it exists.
//
export const WSAEPFNOSUPPORT = 10046;

//
// MessageId: WSAEAFNOSUPPORT
//
// MessageText:
//
//  An address incompatible with the requested protocol was used.
//
export const WSAEAFNOSUPPORT = 10047;

//
// MessageId: WSAEADDRINUSE
//
// MessageText:
//
//  Only one usage of each socket address (protocol/network address/port) is normally permitted.
//
export const WSAEADDRINUSE = 10048;

//
// MessageId: WSAEADDRNOTAVAIL
//
// MessageText:
//
//  The requested address is not valid in its context.
//
export const WSAEADDRNOTAVAIL = 10049;

//
// MessageId: WSAENETDOWN
//
// MessageText:
//
//  A socket operation encountered a dead network.
//
export const WSAENETDOWN = 10050;

//
// MessageId: WSAENETUNREACH
//
// MessageText:
//
//  A socket operation was attempted to an unreachable network.
//
export const WSAENETUNREACH = 10051;

//
// MessageId: WSAENETRESET
//
// MessageText:
//
//  The connection has been broken due to keep-alive activity detecting a failure while the operation was in progress.
//
export const WSAENETRESET = 10052;

//
// MessageId: WSAECONNABORTED
//
// MessageText:
//
//  An established connection was aborted by the software in your host machine.
//
export const WSAECONNABORTED = 10053;

//
// MessageId: WSAECONNRESET
//
// MessageText:
//
//  An existing connection was forcibly closed by the remote host.
//
export const WSAECONNRESET = 10054;

//
// MessageId: WSAENOBUFS
//
// MessageText:
//
//  An operation on a socket could not be performed because the system lacked sufficient buffer space or because a queue was full.
//
export const WSAENOBUFS = 10055;

//
// MessageId: WSAEISCONN
//
// MessageText:
//
//  A connect request was made on an already connected socket.
//
export const WSAEISCONN = 10056;

//
// MessageId: WSAENOTCONN
//
// MessageText:
//
//  A request to send or receive data was disallowed because the socket is not connected and (when sending on a datagram socket using a sendto call) no address was supplied.
//
export const WSAENOTCONN = 10057;

//
// MessageId: WSAESHUTDOWN
//
// MessageText:
//
//  A request to send or receive data was disallowed because the socket had already been shut down in that direction with a previous shutdown call.
//
export const WSAESHUTDOWN = 10058;

//
// MessageId: WSAETOOMANYREFS
//
// MessageText:
//
//  Too many references to some kernel object.
//
export const WSAETOOMANYREFS = 10059;

//
// MessageId: WSAETIMEDOUT
//
// MessageText:
//
//  A connection attempt failed because the connected party did not properly respond after a period of time, or established connection failed because connected host has failed to respond.
//
export const WSAETIMEDOUT = 10060;

//
// MessageId: WSAECONNREFUSED
//
// MessageText:
//
//  No connection could be made because the target machine actively refused it.
//
export const WSAECONNREFUSED = 10061;

//
// MessageId: WSAELOOP
//
// MessageText:
//
//  Cannot translate name.
//
export const WSAELOOP = 10062;

//
// MessageId: WSAENAMETOOLONG
//
// MessageText:
//
//  Name component or name was too long.
//
export const WSAENAMETOOLONG = 10063;

//
// MessageId: WSAEHOSTDOWN
//
// MessageText:
//
//  A socket operation failed because the destination host was down.
//
export const WSAEHOSTDOWN = 10064;

//
// MessageId: WSAEHOSTUNREACH
//
// MessageText:
//
//  A socket operation was attempted to an unreachable host.
//
export const WSAEHOSTUNREACH = 10065;

//
// MessageId: WSAENOTEMPTY
//
// MessageText:
//
//  Cannot remove a directory that is not empty.
//
export const WSAENOTEMPTY = 10066;

//
// MessageId: WSAEPROCLIM
//
// MessageText:
//
//  A Windows Sockets implementation may have a limit on the number of applications that may use it simultaneously.
//
export const WSAEPROCLIM = 10067;

//
// MessageId: WSAEUSERS
//
// MessageText:
//
//  Ran out of quota.
//
export const WSAEUSERS = 10068;

//
// MessageId: WSAEDQUOT
//
// MessageText:
//
//  Ran out of disk quota.
//
export const WSAEDQUOT = 10069;

//
// MessageId: WSAESTALE
//
// MessageText:
//
//  File handle reference is no longer available.
//
export const WSAESTALE = 10070;

//
// MessageId: WSAEREMOTE
//
// MessageText:
//
//  Item is not available locally.
//
export const WSAEREMOTE = 10071;

//
// MessageId: WSASYSNOTREADY
//
// MessageText:
//
//  WSAStartup cannot function at this time because the underlying system it uses to provide network services is currently unavailable.
//
export const WSASYSNOTREADY = 10091;

//
// MessageId: WSAVERNOTSUPPORTED
//
// MessageText:
//
//  The Windows Sockets version requested is not supported.
//
export const WSAVERNOTSUPPORTED = 10092;

//
// MessageId: WSANOTINITIALISED
//
// MessageText:
//
//  Either the application has not called WSAStartup, or WSAStartup failed.
//
export const WSANOTINITIALISED = 10093;

//
// MessageId: WSAEDISCON
//
// MessageText:
//
//  Returned by WSARecv or WSARecvFrom to indicate the remote party has initiated a graceful shutdown sequence.
//
export const WSAEDISCON = 10101;

//
// MessageId: WSAENOMORE
//
// MessageText:
//
//  No more results can be returned by WSALookupServiceNext.
//
export const WSAENOMORE = 10102;

//
// MessageId: WSAECANCELLED
//
// MessageText:
//
//  A call to WSALookupServiceEnd was made while this call was still processing. The call has been canceled.
//
export const WSAECANCELLED = 10103;

//
// MessageId: WSAEINVALIDPROCTABLE
//
// MessageText:
//
//  The procedure call table is invalid.
//
export const WSAEINVALIDPROCTABLE = 10104;

//
// MessageId: WSAEINVALIDPROVIDER
//
// MessageText:
//
//  The requested service provider is invalid.
//
export const WSAEINVALIDPROVIDER = 10105;

//
// MessageId: WSAEPROVIDERFAILEDINIT
//
// MessageText:
//
//  The requested service provider could not be loaded or initialized.
//
export const WSAEPROVIDERFAILEDINIT = 10106;

//
// MessageId: WSASYSCALLFAILURE
//
// MessageText:
//
//  A system call that should never fail has failed.
//
export const WSASYSCALLFAILURE = 10107;

//
// MessageId: WSASERVICE_NOT_FOUND
//
// MessageText:
//
//  No such service is known. The service cannot be found in the specified name space.
//
export const WSASERVICE_NOT_FOUND = 10108;

//
// MessageId: WSATYPE_NOT_FOUND
//
// MessageText:
//
//  The specified class was not found.
//
export const WSATYPE_NOT_FOUND = 10109;

//
// MessageId: WSA_E_NO_MORE
//
// MessageText:
//
//  No more results can be returned by WSALookupServiceNext.
//
export const WSA_E_NO_MORE = 10110;

//
// MessageId: WSA_E_CANCELLED
//
// MessageText:
//
//  A call to WSALookupServiceEnd was made while this call was still processing. The call has been canceled.
//
export const WSA_E_CANCELLED = 10111;

//
// MessageId: WSAEREFUSED
//
// MessageText:
//
//  A database query failed because it was actively refused.
//
export const WSAEREFUSED = 10112;

//
// MessageId: WSAHOST_NOT_FOUND
//
// MessageText:
//
//  No such host is known.
//
export const WSAHOST_NOT_FOUND = 11001;

//
// MessageId: WSATRY_AGAIN
//
// MessageText:
//
//  This is usually a temporary error during hostname resolution and means that the local server did not receive a response from an authoritative server.
//
export const WSATRY_AGAIN = 11002;

//
// MessageId: WSANO_RECOVERY
//
// MessageText:
//
//  A non-recoverable error occurred during a database lookup.
//
export const WSANO_RECOVERY = 11003;

//
// MessageId: WSANO_DATA
//
// MessageText:
//
//  The requested name is valid, but no data of the requested type was found.
//
export const WSANO_DATA = 11004;

//
// MessageId: WSA_QOS_RECEIVERS
//
// MessageText:
//
//  At least one reserve has arrived.
//
export const WSA_QOS_RECEIVERS = 11005;

//
// MessageId: WSA_QOS_SENDERS
//
// MessageText:
//
//  At least one path has arrived.
//
export const WSA_QOS_SENDERS = 11006;

//
// MessageId: WSA_QOS_NO_SENDERS
//
// MessageText:
//
//  There are no senders.
//
export const WSA_QOS_NO_SENDERS = 11007;

//
// MessageId: WSA_QOS_NO_RECEIVERS
//
// MessageText:
//
//  There are no receivers.
//
export const WSA_QOS_NO_RECEIVERS = 11008;

//
// MessageId: WSA_QOS_REQUEST_CONFIRMED
//
// MessageText:
//
//  Reserve has been confirmed.
//
export const WSA_QOS_REQUEST_CONFIRMED = 11009;

//
// MessageId: WSA_QOS_ADMISSION_FAILURE
//
// MessageText:
//
//  Error due to lack of resources.
//
export const WSA_QOS_ADMISSION_FAILURE = 11010;

//
// MessageId: WSA_QOS_POLICY_FAILURE
//
// MessageText:
//
//  Rejected for administrative reasons - bad credentials.
//
export const WSA_QOS_POLICY_FAILURE = 11011;

//
// MessageId: WSA_QOS_BAD_STYLE
//
// MessageText:
//
//  Unknown or conflicting style.
//
export const WSA_QOS_BAD_STYLE = 11012;

//
// MessageId: WSA_QOS_BAD_OBJECT
//
// MessageText:
//
//  Problem with some part of the filterspec or providerspecific buffer in general.
//
export const WSA_QOS_BAD_OBJECT = 11013;

//
// MessageId: WSA_QOS_TRAFFIC_CTRL_ERROR
//
// MessageText:
//
//  Problem with some part of the flowspec.
//
export const WSA_QOS_TRAFFIC_CTRL_ERROR = 11014;

//
// MessageId: WSA_QOS_GENERIC_ERROR
//
// MessageText:
//
//  General QOS error.
//
export const WSA_QOS_GENERIC_ERROR = 11015;

//
// MessageId: WSA_QOS_ESERVICETYPE
//
// MessageText:
//
//  An invalid or unrecognized service type was found in the flowspec.
//
export const WSA_QOS_ESERVICETYPE = 11016;

//
// MessageId: WSA_QOS_EFLOWSPEC
//
// MessageText:
//
//  An invalid or inconsistent flowspec was found in the QOS structure.
//
export const WSA_QOS_EFLOWSPEC = 11017;

//
// MessageId: WSA_QOS_EPROVSPECBUF
//
// MessageText:
//
//  Invalid QOS provider-specific buffer.
//
export const WSA_QOS_EPROVSPECBUF = 11018;

//
// MessageId: WSA_QOS_EFILTERSTYLE
//
// MessageText:
//
//  An invalid QOS filter style was used.
//
export const WSA_QOS_EFILTERSTYLE = 11019;

//
// MessageId: WSA_QOS_EFILTERTYPE
//
// MessageText:
//
//  An invalid QOS filter type was used.
//
export const WSA_QOS_EFILTERTYPE = 11020;

//
// MessageId: WSA_QOS_EFILTERCOUNT
//
// MessageText:
//
//  An incorrect number of QOS FILTERSPECs were specified in the FLOWDESCRIPTOR.
//
export const WSA_QOS_EFILTERCOUNT = 11021;

//
// MessageId: WSA_QOS_EOBJLENGTH
//
// MessageText:
//
//  An object with an invalid ObjectLength field was specified in the QOS provider-specific buffer.
//
export const WSA_QOS_EOBJLENGTH = 11022;

//
// MessageId: WSA_QOS_EFLOWCOUNT
//
// MessageText:
//
//  An incorrect number of flow descriptors was specified in the QOS structure.
//
export const WSA_QOS_EFLOWCOUNT = 11023;

//
// MessageId: WSA_QOS_EUNKOWNPSOBJ
//
// MessageText:
//
//  An unrecognized object was found in the QOS provider-specific buffer.
//
export const WSA_QOS_EUNKOWNPSOBJ = 11024;

//
// MessageId: WSA_QOS_EPOLICYOBJ
//
// MessageText:
//
//  An invalid policy object was found in the QOS provider-specific buffer.
//
export const WSA_QOS_EPOLICYOBJ = 11025;

//
// MessageId: WSA_QOS_EFLOWDESC
//
// MessageText:
//
//  An invalid QOS flow descriptor was found in the flow descriptor list.
//
export const WSA_QOS_EFLOWDESC = 11026;

//
// MessageId: WSA_QOS_EPSFLOWSPEC
//
// MessageText:
//
//  An invalid or inconsistent flowspec was found in the QOS provider specific buffer.
//
export const WSA_QOS_EPSFLOWSPEC = 11027;

//
// MessageId: WSA_QOS_EPSFILTERSPEC
//
// MessageText:
//
//  An invalid FILTERSPEC was found in the QOS provider-specific buffer.
//
export const WSA_QOS_EPSFILTERSPEC = 11028;

//
// MessageId: WSA_QOS_ESDMODEOBJ
//
// MessageText:
//
//  An invalid shape discard mode object was found in the QOS provider specific buffer.
//
export const WSA_QOS_ESDMODEOBJ = 11029;

//
// MessageId: WSA_QOS_ESHAPERATEOBJ
//
// MessageText:
//
//  An invalid shaping rate object was found in the QOS provider-specific buffer.
//
export const WSA_QOS_ESHAPERATEOBJ = 11030;

//
// MessageId: WSA_QOS_RESERVED_PETYPE
//
// MessageText:
//
//  A reserved policy element was found in the QOS provider-specific buffer.
//
export const WSA_QOS_RESERVED_PETYPE = 11031;

///////////////////////////////////////////////////
//                                               //
//           End of WinSock Error Codes          //
//                                               //
//                 10000 to 11999                //
///////////////////////////////////////////////////

///////////////////////////////////////////////////
//                                               //
//             Side By Side Error Codes          //
//                                               //
//                 14000 to 14999                //
///////////////////////////////////////////////////

//
// MessageId: ERROR_SXS_SECTION_NOT_FOUND
//
// MessageText:
//
//  The requested section was not present in the activation context.
//
export const ERROR_SXS_SECTION_NOT_FOUND = 14000;

//
// MessageId: ERROR_SXS_CANT_GEN_ACTCTX
//
// MessageText:
//
//  This application has failed to start because the application configuration is incorrect. Reinstalling the application may fix this problem.
//
export const ERROR_SXS_CANT_GEN_ACTCTX = 14001;

//
// MessageId: ERROR_SXS_INVALID_ACTCTXDATA_FORMAT
//
// MessageText:
//
//  The application binding data format is invalid.
//
export const ERROR_SXS_INVALID_ACTCTXDATA_FORMAT = 14002;

//
// MessageId: ERROR_SXS_ASSEMBLY_NOT_FOUND
//
// MessageText:
//
//  The referenced assembly is not installed on your system.
//
export const ERROR_SXS_ASSEMBLY_NOT_FOUND = 14003;

//
// MessageId: ERROR_SXS_MANIFEST_FORMAT_ERROR
//
// MessageText:
//
//  The manifest file does not begin with the required tag and format information.
//
export const ERROR_SXS_MANIFEST_FORMAT_ERROR = 14004;

//
// MessageId: ERROR_SXS_MANIFEST_PARSE_ERROR
//
// MessageText:
//
//  The manifest file contains one or more syntax errors.
//
export const ERROR_SXS_MANIFEST_PARSE_ERROR = 14005;

//
// MessageId: ERROR_SXS_ACTIVATION_CONTEXT_DISABLED
//
// MessageText:
//
//  The application attempted to activate a disabled activation context.
//
export const ERROR_SXS_ACTIVATION_CONTEXT_DISABLED = 14006;

//
// MessageId: ERROR_SXS_KEY_NOT_FOUND
//
// MessageText:
//
//  The requested lookup key was not found in any active activation context.
//
export const ERROR_SXS_KEY_NOT_FOUND = 14007;

//
// MessageId: ERROR_SXS_VERSION_CONFLICT
//
// MessageText:
//
//  A component version required by the application conflicts with another component version already active.
//
export const ERROR_SXS_VERSION_CONFLICT = 14008;

//
// MessageId: ERROR_SXS_WRONG_SECTION_TYPE
//
// MessageText:
//
//  The type requested activation context section does not match the query API used.
//
export const ERROR_SXS_WRONG_SECTION_TYPE = 14009;

//
// MessageId: ERROR_SXS_THREAD_QUERIES_DISABLED
//
// MessageText:
//
//  Lack of system resources has required isolated activation to be disabled for the current thread of execution.
//
export const ERROR_SXS_THREAD_QUERIES_DISABLED = 14010;

//
// MessageId: ERROR_SXS_PROCESS_DEFAULT_ALREADY_SET
//
// MessageText:
//
//  An attempt to set the process default activation context failed because the process default activation context was already set.
//
export const ERROR_SXS_PROCESS_DEFAULT_ALREADY_SET = 14011;

//
// MessageId: ERROR_SXS_UNKNOWN_ENCODING_GROUP
//
// MessageText:
//
//  The encoding group identifier specified is not recognized.
//
export const ERROR_SXS_UNKNOWN_ENCODING_GROUP = 14012;

//
// MessageId: ERROR_SXS_UNKNOWN_ENCODING
//
// MessageText:
//
//  The encoding requested is not recognized.
//
export const ERROR_SXS_UNKNOWN_ENCODING = 14013;

//
// MessageId: ERROR_SXS_INVALID_XML_NAMESPACE_URI
//
// MessageText:
//
//  The manifest contains a reference to an invalid URI.
//
export const ERROR_SXS_INVALID_XML_NAMESPACE_URI = 14014;

//
// MessageId: ERROR_SXS_ROOT_MANIFEST_DEPENDENCY_NOT_INSTALLED
//
// MessageText:
//
//  The application manifest contains a reference to a dependent assembly which is not installed
//
export const ERROR_SXS_ROOT_MANIFEST_DEPENDENCY_NOT_INSTALLED = 14015;

//
// MessageId: ERROR_SXS_LEAF_MANIFEST_DEPENDENCY_NOT_INSTALLED
//
// MessageText:
//
//  The manifest for an assembly used by the application has a reference to a dependent assembly which is not installed
//
export const ERROR_SXS_LEAF_MANIFEST_DEPENDENCY_NOT_INSTALLED = 14016;

//
// MessageId: ERROR_SXS_INVALID_ASSEMBLY_IDENTITY_ATTRIBUTE
//
// MessageText:
//
//  The manifest contains an attribute for the assembly identity which is not valid.
//
export const ERROR_SXS_INVALID_ASSEMBLY_IDENTITY_ATTRIBUTE = 14017;

//
// MessageId: ERROR_SXS_MANIFEST_MISSING_REQUIRED_DEFAULT_NAMESPACE
//
// MessageText:
//
//  The manifest is missing the required default namespace specification on the assembly element.
//
export const ERROR_SXS_MANIFEST_MISSING_REQUIRED_DEFAULT_NAMESPACE = 14018;

//
// MessageId: ERROR_SXS_MANIFEST_INVALID_REQUIRED_DEFAULT_NAMESPACE
//
// MessageText:
//
//  The manifest has a default namespace specified on the assembly element but its value is not "urn:schemas-microsoft-com:asm.v1".
//
export const ERROR_SXS_MANIFEST_INVALID_REQUIRED_DEFAULT_NAMESPACE = 14019;

//
// MessageId: ERROR_SXS_PRIVATE_MANIFEST_CROSS_PATH_WITH_REPARSE_POINT
//
// MessageText:
//
//  The private manifest probed has crossed reparse-point-associated path
//
export const ERROR_SXS_PRIVATE_MANIFEST_CROSS_PATH_WITH_REPARSE_POINT = 14020;

//
// MessageId: ERROR_SXS_DUPLICATE_DLL_NAME
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest have files by the same name.
//
export const ERROR_SXS_DUPLICATE_DLL_NAME = 14021;

//
// MessageId: ERROR_SXS_DUPLICATE_WINDOWCLASS_NAME
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest have window classes with the same name.
//
export const ERROR_SXS_DUPLICATE_WINDOWCLASS_NAME = 14022;

//
// MessageId: ERROR_SXS_DUPLICATE_CLSID
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest have the same COM server CLSIDs.
//
export const ERROR_SXS_DUPLICATE_CLSID = 14023;

//
// MessageId: ERROR_SXS_DUPLICATE_IID
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest have proxies for the same COM interface IIDs.
//
export const ERROR_SXS_DUPLICATE_IID = 14024;

//
// MessageId: ERROR_SXS_DUPLICATE_TLBID
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest have the same COM type library TLBIDs.
//
export const ERROR_SXS_DUPLICATE_TLBID = 14025;

//
// MessageId: ERROR_SXS_DUPLICATE_PROGID
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest have the same COM ProgIDs.
//
export const ERROR_SXS_DUPLICATE_PROGID = 14026;

//
// MessageId: ERROR_SXS_DUPLICATE_ASSEMBLY_NAME
//
// MessageText:
//
//  Two or more components referenced directly or indirectly by the application manifest are different versions of the same component which is not permitted.
//
export const ERROR_SXS_DUPLICATE_ASSEMBLY_NAME = 14027;

//
// MessageId: ERROR_SXS_FILE_HASH_MISMATCH
//
// MessageText:
//
//  A component's file does not match the verification information present in the
//  component manifest.
//
export const ERROR_SXS_FILE_HASH_MISMATCH = 14028;

//
// MessageId: ERROR_SXS_POLICY_PARSE_ERROR
//
// MessageText:
//
//  The policy manifest contains one or more syntax errors.
//
export const ERROR_SXS_POLICY_PARSE_ERROR = 14029;

//
// MessageId: ERROR_SXS_XML_E_MISSINGQUOTE
//
// MessageText:
//
//  Manifest Parse Error : A string literal was expected, but no opening quote character was found.
//
export const ERROR_SXS_XML_E_MISSINGQUOTE = 14030;

//
// MessageId: ERROR_SXS_XML_E_COMMENTSYNTAX
//
// MessageText:
//
//  Manifest Parse Error : Incorrect syntax was used in a comment.
//
export const ERROR_SXS_XML_E_COMMENTSYNTAX = 14031;

//
// MessageId: ERROR_SXS_XML_E_BADSTARTNAMECHAR
//
// MessageText:
//
//  Manifest Parse Error : A name was started with an invalid character.
//
export const ERROR_SXS_XML_E_BADSTARTNAMECHAR = 14032;

//
// MessageId: ERROR_SXS_XML_E_BADNAMECHAR
//
// MessageText:
//
//  Manifest Parse Error : A name contained an invalid character.
//
export const ERROR_SXS_XML_E_BADNAMECHAR = 14033;

//
// MessageId: ERROR_SXS_XML_E_BADCHARINSTRING
//
// MessageText:
//
//  Manifest Parse Error : A string literal contained an invalid character.
//
export const ERROR_SXS_XML_E_BADCHARINSTRING = 14034;

//
// MessageId: ERROR_SXS_XML_E_XMLDECLSYNTAX
//
// MessageText:
//
//  Manifest Parse Error : Invalid syntax for an xml declaration.
//
export const ERROR_SXS_XML_E_XMLDECLSYNTAX = 14035;

//
// MessageId: ERROR_SXS_XML_E_BADCHARDATA
//
// MessageText:
//
//  Manifest Parse Error : An Invalid character was found in text content.
//
export const ERROR_SXS_XML_E_BADCHARDATA = 14036;

//
// MessageId: ERROR_SXS_XML_E_MISSINGWHITESPACE
//
// MessageText:
//
//  Manifest Parse Error : Required white space was missing.
//
export const ERROR_SXS_XML_E_MISSINGWHITESPACE = 14037;

//
// MessageId: ERROR_SXS_XML_E_EXPECTINGTAGEND
//
// MessageText:
//
//  Manifest Parse Error : The character '>' was expected.
//
export const ERROR_SXS_XML_E_EXPECTINGTAGEND = 14038;

//
// MessageId: ERROR_SXS_XML_E_MISSINGSEMICOLON
//
// MessageText:
//
//  Manifest Parse Error : A semi colon character was expected.
//
export const ERROR_SXS_XML_E_MISSINGSEMICOLON = 14039;

//
// MessageId: ERROR_SXS_XML_E_UNBALANCEDPAREN
//
// MessageText:
//
//  Manifest Parse Error : Unbalanced parentheses.
//
export const ERROR_SXS_XML_E_UNBALANCEDPAREN = 14040;

//
// MessageId: ERROR_SXS_XML_E_INTERNALERROR
//
// MessageText:
//
//  Manifest Parse Error : Internal error.
//
export const ERROR_SXS_XML_E_INTERNALERROR = 14041;

//
// MessageId: ERROR_SXS_XML_E_UNEXPECTED_WHITESPACE
//
// MessageText:
//
//  Manifest Parse Error : Whitespace is not allowed at this location.
//
export const ERROR_SXS_XML_E_UNEXPECTED_WHITESPACE = 14042;

//
// MessageId: ERROR_SXS_XML_E_INCOMPLETE_ENCODING
//
// MessageText:
//
//  Manifest Parse Error : End of file reached in invalid state for current encoding.
//
export const ERROR_SXS_XML_E_INCOMPLETE_ENCODING = 14043;

//
// MessageId: ERROR_SXS_XML_E_MISSING_PAREN
//
// MessageText:
//
//  Manifest Parse Error : Missing parenthesis.
//
export const ERROR_SXS_XML_E_MISSING_PAREN = 14044;

//
// MessageId: ERROR_SXS_XML_E_EXPECTINGCLOSEQUOTE
//
// MessageText:
//
//  Manifest Parse Error : A single or double closing quote character (\' or \") is missing.
//
export const ERROR_SXS_XML_E_EXPECTINGCLOSEQUOTE = 14045;

//
// MessageId: ERROR_SXS_XML_E_MULTIPLE_COLONS
//
// MessageText:
//
//  Manifest Parse Error : Multiple colons are not allowed in a name.
//
export const ERROR_SXS_XML_E_MULTIPLE_COLONS = 14046;

//
// MessageId: ERROR_SXS_XML_E_INVALID_DECIMAL
//
// MessageText:
//
//  Manifest Parse Error : Invalid character for decimal digit.
//
export const ERROR_SXS_XML_E_INVALID_DECIMAL = 14047;

//
// MessageId: ERROR_SXS_XML_E_INVALID_HEXIDECIMAL
//
// MessageText:
//
//  Manifest Parse Error : Invalid character for hexadecimal digit.
//
export const ERROR_SXS_XML_E_INVALID_HEXIDECIMAL = 14048;

//
// MessageId: ERROR_SXS_XML_E_INVALID_UNICODE
//
// MessageText:
//
//  Manifest Parse Error : Invalid unicode character value for this platform.
//
export const ERROR_SXS_XML_E_INVALID_UNICODE = 14049;

//
// MessageId: ERROR_SXS_XML_E_WHITESPACEORQUESTIONMARK
//
// MessageText:
//
//  Manifest Parse Error : Expecting whitespace or '?'.
//
export const ERROR_SXS_XML_E_WHITESPACEORQUESTIONMARK = 14050;

//
// MessageId: ERROR_SXS_XML_E_UNEXPECTEDENDTAG
//
// MessageText:
//
//  Manifest Parse Error : End tag was not expected at this location.
//
export const ERROR_SXS_XML_E_UNEXPECTEDENDTAG = 14051;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDTAG
//
// MessageText:
//
//  Manifest Parse Error : The following tags were not closed: %1.
//
export const ERROR_SXS_XML_E_UNCLOSEDTAG = 14052;

//
// MessageId: ERROR_SXS_XML_E_DUPLICATEATTRIBUTE
//
// MessageText:
//
//  Manifest Parse Error : Duplicate attribute.
//
export const ERROR_SXS_XML_E_DUPLICATEATTRIBUTE = 14053;

//
// MessageId: ERROR_SXS_XML_E_MULTIPLEROOTS
//
// MessageText:
//
//  Manifest Parse Error : Only one top level element is allowed in an XML document.
//
export const ERROR_SXS_XML_E_MULTIPLEROOTS = 14054;

//
// MessageId: ERROR_SXS_XML_E_INVALIDATROOTLEVEL
//
// MessageText:
//
//  Manifest Parse Error : Invalid at the top level of the document.
//
export const ERROR_SXS_XML_E_INVALIDATROOTLEVEL = 14055;

//
// MessageId: ERROR_SXS_XML_E_BADXMLDECL
//
// MessageText:
//
//  Manifest Parse Error : Invalid xml declaration.
//
export const ERROR_SXS_XML_E_BADXMLDECL = 14056;

//
// MessageId: ERROR_SXS_XML_E_MISSINGROOT
//
// MessageText:
//
//  Manifest Parse Error : XML document must have a top level element.
//
export const ERROR_SXS_XML_E_MISSINGROOT = 14057;

//
// MessageId: ERROR_SXS_XML_E_UNEXPECTEDEOF
//
// MessageText:
//
//  Manifest Parse Error : Unexpected end of file.
//
export const ERROR_SXS_XML_E_UNEXPECTEDEOF = 14058;

//
// MessageId: ERROR_SXS_XML_E_BADPEREFINSUBSET
//
// MessageText:
//
//  Manifest Parse Error : Parameter entities cannot be used inside markup declarations in an internal subset.
//
export const ERROR_SXS_XML_E_BADPEREFINSUBSET = 14059;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDSTARTTAG
//
// MessageText:
//
//  Manifest Parse Error : Element was not closed.
//
export const ERROR_SXS_XML_E_UNCLOSEDSTARTTAG = 14060;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDENDTAG
//
// MessageText:
//
//  Manifest Parse Error : End element was missing the character '>'.
//
export const ERROR_SXS_XML_E_UNCLOSEDENDTAG = 14061;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDSTRING
//
// MessageText:
//
//  Manifest Parse Error : A string literal was not closed.
//
export const ERROR_SXS_XML_E_UNCLOSEDSTRING = 14062;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDCOMMENT
//
// MessageText:
//
//  Manifest Parse Error : A comment was not closed.
//
export const ERROR_SXS_XML_E_UNCLOSEDCOMMENT = 14063;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDDECL
//
// MessageText:
//
//  Manifest Parse Error : A declaration was not closed.
//
export const ERROR_SXS_XML_E_UNCLOSEDDECL = 14064;

//
// MessageId: ERROR_SXS_XML_E_UNCLOSEDCDATA
//
// MessageText:
//
//  Manifest Parse Error : A CDATA section was not closed.
//
export const ERROR_SXS_XML_E_UNCLOSEDCDATA = 14065;

//
// MessageId: ERROR_SXS_XML_E_RESERVEDNAMESPACE
//
// MessageText:
//
//  Manifest Parse Error : The namespace prefix is not allowed to start with the reserved string "xml".
//
export const ERROR_SXS_XML_E_RESERVEDNAMESPACE = 14066;

//
// MessageId: ERROR_SXS_XML_E_INVALIDENCODING
//
// MessageText:
//
//  Manifest Parse Error : System does not support the specified encoding.
//
export const ERROR_SXS_XML_E_INVALIDENCODING = 14067;

//
// MessageId: ERROR_SXS_XML_E_INVALIDSWITCH
//
// MessageText:
//
//  Manifest Parse Error : Switch from current encoding to specified encoding not supported.
//
export const ERROR_SXS_XML_E_INVALIDSWITCH = 14068;

//
// MessageId: ERROR_SXS_XML_E_BADXMLCASE
//
// MessageText:
//
//  Manifest Parse Error : The name 'xml' is reserved and must be lower case.
//
export const ERROR_SXS_XML_E_BADXMLCASE = 14069;

//
// MessageId: ERROR_SXS_XML_E_INVALID_STANDALONE
//
// MessageText:
//
//  Manifest Parse Error : The standalone attribute must have the value 'yes' or 'no'.
//
export const ERROR_SXS_XML_E_INVALID_STANDALONE = 14070;

//
// MessageId: ERROR_SXS_XML_E_UNEXPECTED_STANDALONE
//
// MessageText:
//
//  Manifest Parse Error : The standalone attribute cannot be used in external entities.
//
export const ERROR_SXS_XML_E_UNEXPECTED_STANDALONE = 14071;

//
// MessageId: ERROR_SXS_XML_E_INVALID_VERSION
//
// MessageText:
//
//  Manifest Parse Error : Invalid version number.
//
export const ERROR_SXS_XML_E_INVALID_VERSION = 14072;

//
// MessageId: ERROR_SXS_XML_E_MISSINGEQUALS
//
// MessageText:
//
//  Manifest Parse Error : Missing equals sign between attribute and attribute value.
//
export const ERROR_SXS_XML_E_MISSINGEQUALS = 14073;

//
// MessageId: ERROR_SXS_PROTECTION_RECOVERY_FAILED
//
// MessageText:
//
//  Assembly Protection Error : Unable to recover the specified assembly.
//
export const ERROR_SXS_PROTECTION_RECOVERY_FAILED = 14074;

//
// MessageId: ERROR_SXS_PROTECTION_PUBLIC_KEY_TOO_SHORT
//
// MessageText:
//
//  Assembly Protection Error : The public key for an assembly was too short to be allowed.
//
export const ERROR_SXS_PROTECTION_PUBLIC_KEY_TOO_SHORT = 14075;

//
// MessageId: ERROR_SXS_PROTECTION_CATALOG_NOT_VALID
//
// MessageText:
//
//  Assembly Protection Error : The catalog for an assembly is not valid, or does not match the assembly's manifest.
//
export const ERROR_SXS_PROTECTION_CATALOG_NOT_VALID = 14076;

//
// MessageId: ERROR_SXS_UNTRANSLATABLE_HRESULT
//
// MessageText:
//
//  An HRESULT could not be translated to a corresponding Win32 error code.
//
export const ERROR_SXS_UNTRANSLATABLE_HRESULT = 14077;

//
// MessageId: ERROR_SXS_PROTECTION_CATALOG_FILE_MISSING
//
// MessageText:
//
//  Assembly Protection Error : The catalog for an assembly is missing.
//
export const ERROR_SXS_PROTECTION_CATALOG_FILE_MISSING = 14078;

//
// MessageId: ERROR_SXS_MISSING_ASSEMBLY_IDENTITY_ATTRIBUTE
//
// MessageText:
//
//  The supplied assembly identity is missing one or more attributes which must be present in this context.
//
export const ERROR_SXS_MISSING_ASSEMBLY_IDENTITY_ATTRIBUTE = 14079;

//
// MessageId: ERROR_SXS_INVALID_ASSEMBLY_IDENTITY_ATTRIBUTE_NAME
//
// MessageText:
//
//  The supplied assembly identity has one or more attribute names that contain characters not permitted in XML names.
//
export const ERROR_SXS_INVALID_ASSEMBLY_IDENTITY_ATTRIBUTE_NAME = 14080;

///////////////////////////////////////////////////
//                                               //
//           End of Side By Side Error Codes     //
//                                               //
//                 14000 to 14999                //
///////////////////////////////////////////////////

///////////////////////////////////////////////////
//                                               //
//           Start of IPSec Error codes          //
//                                               //
//                 13000 to 13999                //
///////////////////////////////////////////////////

//
// MessageId: ERROR_IPSEC_QM_POLICY_EXISTS
//
// MessageText:
//
//  The specified quick mode policy already exists.
//
export const ERROR_IPSEC_QM_POLICY_EXISTS = 13000;

//
// MessageId: ERROR_IPSEC_QM_POLICY_NOT_FOUND
//
// MessageText:
//
//  The specified quick mode policy was not found.
//
export const ERROR_IPSEC_QM_POLICY_NOT_FOUND = 13001;

//
// MessageId: ERROR_IPSEC_QM_POLICY_IN_USE
//
// MessageText:
//
//  The specified quick mode policy is being used.
//
export const ERROR_IPSEC_QM_POLICY_IN_USE = 13002;

//
// MessageId: ERROR_IPSEC_MM_POLICY_EXISTS
//
// MessageText:
//
//  The specified main mode policy already exists.
//
export const ERROR_IPSEC_MM_POLICY_EXISTS = 13003;

//
// MessageId: ERROR_IPSEC_MM_POLICY_NOT_FOUND
//
// MessageText:
//
//  The specified main mode policy was not found
//
export const ERROR_IPSEC_MM_POLICY_NOT_FOUND = 13004;

//
// MessageId: ERROR_IPSEC_MM_POLICY_IN_USE
//
// MessageText:
//
//  The specified main mode policy is being used.
//
export const ERROR_IPSEC_MM_POLICY_IN_USE = 13005;

//
// MessageId: ERROR_IPSEC_MM_FILTER_EXISTS
//
// MessageText:
//
//  The specified main mode filter already exists.
//
export const ERROR_IPSEC_MM_FILTER_EXISTS = 13006;

//
// MessageId: ERROR_IPSEC_MM_FILTER_NOT_FOUND
//
// MessageText:
//
//  The specified main mode filter was not found.
//
export const ERROR_IPSEC_MM_FILTER_NOT_FOUND = 13007;

//
// MessageId: ERROR_IPSEC_TRANSPORT_FILTER_EXISTS
//
// MessageText:
//
//  The specified transport mode filter already exists.
//
export const ERROR_IPSEC_TRANSPORT_FILTER_EXISTS = 13008;

//
// MessageId: ERROR_IPSEC_TRANSPORT_FILTER_NOT_FOUND
//
// MessageText:
//
//  The specified transport mode filter does not exist.
//
export const ERROR_IPSEC_TRANSPORT_FILTER_NOT_FOUND = 13009;

//
// MessageId: ERROR_IPSEC_MM_AUTH_EXISTS
//
// MessageText:
//
//  The specified main mode authentication list exists.
//
export const ERROR_IPSEC_MM_AUTH_EXISTS = 13010;

//
// MessageId: ERROR_IPSEC_MM_AUTH_NOT_FOUND
//
// MessageText:
//
//  The specified main mode authentication list was not found.
//
export const ERROR_IPSEC_MM_AUTH_NOT_FOUND = 13011;

//
// MessageId: ERROR_IPSEC_MM_AUTH_IN_USE
//
// MessageText:
//
//  The specified quick mode policy is being used.
//
export const ERROR_IPSEC_MM_AUTH_IN_USE = 13012;

//
// MessageId: ERROR_IPSEC_DEFAULT_MM_POLICY_NOT_FOUND
//
// MessageText:
//
//  The specified main mode policy was not found.
//
export const ERROR_IPSEC_DEFAULT_MM_POLICY_NOT_FOUND = 13013;

//
// MessageId: ERROR_IPSEC_DEFAULT_MM_AUTH_NOT_FOUND
//
// MessageText:
//
//  The specified quick mode policy was not found
//
export const ERROR_IPSEC_DEFAULT_MM_AUTH_NOT_FOUND = 13014;

//
// MessageId: ERROR_IPSEC_DEFAULT_QM_POLICY_NOT_FOUND
//
// MessageText:
//
//  The manifest file contains one or more syntax errors.
//
export const ERROR_IPSEC_DEFAULT_QM_POLICY_NOT_FOUND = 13015;

//
// MessageId: ERROR_IPSEC_TUNNEL_FILTER_EXISTS
//
// MessageText:
//
//  The application attempted to activate a disabled activation context.
//
export const ERROR_IPSEC_TUNNEL_FILTER_EXISTS = 13016;

//
// MessageId: ERROR_IPSEC_TUNNEL_FILTER_NOT_FOUND
//
// MessageText:
//
//  The requested lookup key was not found in any active activation context.
//
export const ERROR_IPSEC_TUNNEL_FILTER_NOT_FOUND = 13017;

//
// MessageId: ERROR_IPSEC_MM_FILTER_PENDING_DELETION
//
// MessageText:
//
//  The Main Mode filter is pending deletion.
//
export const ERROR_IPSEC_MM_FILTER_PENDING_DELETION = 13018;

//
// MessageId: ERROR_IPSEC_TRANSPORT_FILTER_PENDING_DELETION
//
// MessageText:
//
//  The transport filter is pending deletion.
//
export const ERROR_IPSEC_TRANSPORT_FILTER_PENDING_DELETION = 13019;

//
// MessageId: ERROR_IPSEC_TUNNEL_FILTER_PENDING_DELETION
//
// MessageText:
//
//  The tunnel filter is pending deletion.
//
export const ERROR_IPSEC_TUNNEL_FILTER_PENDING_DELETION = 13020;

//
// MessageId: ERROR_IPSEC_MM_POLICY_PENDING_DELETION
//
// MessageText:
//
//  The Main Mode policy is pending deletion.
//
export const ERROR_IPSEC_MM_POLICY_PENDING_DELETION = 13021;

//
// MessageId: ERROR_IPSEC_MM_AUTH_PENDING_DELETION
//
// MessageText:
//
//  The Main Mode authentication bundle is pending deletion.
//
export const ERROR_IPSEC_MM_AUTH_PENDING_DELETION = 13022;

//
// MessageId: ERROR_IPSEC_QM_POLICY_PENDING_DELETION
//
// MessageText:
//
//  The Quick Mode policy is pending deletion.
//
export const ERROR_IPSEC_QM_POLICY_PENDING_DELETION = 13023;

//
// MessageId: WARNING_IPSEC_MM_POLICY_PRUNED
//
// MessageText:
//
//  The Main Mode policy was successfully added, but some of the requested offers are not supported.
//
export const WARNING_IPSEC_MM_POLICY_PRUNED = 13024;

//
// MessageId: WARNING_IPSEC_QM_POLICY_PRUNED
//
// MessageText:
//
//  The Quick Mode policy was successfully added, but some of the requested offers are not supported.
//
export const WARNING_IPSEC_QM_POLICY_PRUNED = 13025;

//
// MessageId: ERROR_IPSEC_IKE_NEG_STATUS_BEGIN
//
// MessageText:
//
//  ERROR_IPSEC_IKE_NEG_STATUS_BEGIN
//
export const ERROR_IPSEC_IKE_NEG_STATUS_BEGIN = 13800;

//
// MessageId: ERROR_IPSEC_IKE_AUTH_FAIL
//
// MessageText:
//
//  IKE authentication credentials are unacceptable
//
export const ERROR_IPSEC_IKE_AUTH_FAIL = 13801;

//
// MessageId: ERROR_IPSEC_IKE_ATTRIB_FAIL
//
// MessageText:
//
//  IKE security attributes are unacceptable
//
export const ERROR_IPSEC_IKE_ATTRIB_FAIL = 13802;

//
// MessageId: ERROR_IPSEC_IKE_NEGOTIATION_PENDING
//
// MessageText:
//
//  IKE Negotiation in progress
//
export const ERROR_IPSEC_IKE_NEGOTIATION_PENDING = 13803;

//
// MessageId: ERROR_IPSEC_IKE_GENERAL_PROCESSING_ERROR
//
// MessageText:
//
//  General processing error
//
export const ERROR_IPSEC_IKE_GENERAL_PROCESSING_ERROR = 13804;

//
// MessageId: ERROR_IPSEC_IKE_TIMED_OUT
//
// MessageText:
//
//  Negotiation timed out
//
export const ERROR_IPSEC_IKE_TIMED_OUT = 13805;

//
// MessageId: ERROR_IPSEC_IKE_NO_CERT
//
// MessageText:
//
//  IKE failed to find valid machine certificate
//
export const ERROR_IPSEC_IKE_NO_CERT = 13806;

//
// MessageId: ERROR_IPSEC_IKE_SA_DELETED
//
// MessageText:
//
//  IKE SA deleted by peer before establishment completed
//
export const ERROR_IPSEC_IKE_SA_DELETED = 13807;

//
// MessageId: ERROR_IPSEC_IKE_SA_REAPED
//
// MessageText:
//
//  IKE SA deleted before establishment completed
//
export const ERROR_IPSEC_IKE_SA_REAPED = 13808;

//
// MessageId: ERROR_IPSEC_IKE_MM_ACQUIRE_DROP
//
// MessageText:
//
//  Negotiation request sat in Queue too long
//
export const ERROR_IPSEC_IKE_MM_ACQUIRE_DROP = 13809;

//
// MessageId: ERROR_IPSEC_IKE_QM_ACQUIRE_DROP
//
// MessageText:
//
//  Negotiation request sat in Queue too long
//
export const ERROR_IPSEC_IKE_QM_ACQUIRE_DROP = 13810;

//
// MessageId: ERROR_IPSEC_IKE_QUEUE_DROP_MM
//
// MessageText:
//
//  Negotiation request sat in Queue too long
//
export const ERROR_IPSEC_IKE_QUEUE_DROP_MM = 13811;

//
// MessageId: ERROR_IPSEC_IKE_QUEUE_DROP_NO_MM
//
// MessageText:
//
//  Negotiation request sat in Queue too long
//
export const ERROR_IPSEC_IKE_QUEUE_DROP_NO_MM = 13812;

//
// MessageId: ERROR_IPSEC_IKE_DROP_NO_RESPONSE
//
// MessageText:
//
//  No response from peer
//
export const ERROR_IPSEC_IKE_DROP_NO_RESPONSE = 13813;

//
// MessageId: ERROR_IPSEC_IKE_MM_DELAY_DROP
//
// MessageText:
//
//  Negotiation took too long
//
export const ERROR_IPSEC_IKE_MM_DELAY_DROP = 13814;

//
// MessageId: ERROR_IPSEC_IKE_QM_DELAY_DROP
//
// MessageText:
//
//  Negotiation took too long
//
export const ERROR_IPSEC_IKE_QM_DELAY_DROP = 13815;

//
// MessageId: ERROR_IPSEC_IKE_ERROR
//
// MessageText:
//
//  Unknown error occurred
//
export const ERROR_IPSEC_IKE_ERROR = 13816;

//
// MessageId: ERROR_IPSEC_IKE_CRL_FAILED
//
// MessageText:
//
//  Certificate Revocation Check failed
//
export const ERROR_IPSEC_IKE_CRL_FAILED = 13817;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_KEY_USAGE
//
// MessageText:
//
//  Invalid certificate key usage
//
export const ERROR_IPSEC_IKE_INVALID_KEY_USAGE = 13818;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_CERT_TYPE
//
// MessageText:
//
//  Invalid certificate type
//
export const ERROR_IPSEC_IKE_INVALID_CERT_TYPE = 13819;

//
// MessageId: ERROR_IPSEC_IKE_NO_PRIVATE_KEY
//
// MessageText:
//
//  No private key associated with machine certificate
//
export const ERROR_IPSEC_IKE_NO_PRIVATE_KEY = 13820;

//
// MessageId: ERROR_IPSEC_IKE_DH_FAIL
//
// MessageText:
//
//  Failure in Diffie-Hellman computation
//
export const ERROR_IPSEC_IKE_DH_FAIL = 13822;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_HEADER
//
// MessageText:
//
//  Invalid header
//
export const ERROR_IPSEC_IKE_INVALID_HEADER = 13824;

//
// MessageId: ERROR_IPSEC_IKE_NO_POLICY
//
// MessageText:
//
//  No policy configured
//
export const ERROR_IPSEC_IKE_NO_POLICY = 13825;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_SIGNATURE
//
// MessageText:
//
//  Failed to verify signature
//
export const ERROR_IPSEC_IKE_INVALID_SIGNATURE = 13826;

//
// MessageId: ERROR_IPSEC_IKE_KERBEROS_ERROR
//
// MessageText:
//
//  Failed to authenticate using kerberos
//
export const ERROR_IPSEC_IKE_KERBEROS_ERROR = 13827;

//
// MessageId: ERROR_IPSEC_IKE_NO_PUBLIC_KEY
//
// MessageText:
//
//  Peer's certificate did not have a public key
//
export const ERROR_IPSEC_IKE_NO_PUBLIC_KEY = 13828;

// These must stay as a unit.
//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR
//
// MessageText:
//
//  Error processing error payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR = 13829;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_SA
//
// MessageText:
//
//  Error processing SA payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_SA = 13830;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_PROP
//
// MessageText:
//
//  Error processing Proposal payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_PROP = 13831;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_TRANS
//
// MessageText:
//
//  Error processing Transform payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_TRANS = 13832;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_KE
//
// MessageText:
//
//  Error processing KE payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_KE = 13833;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_ID
//
// MessageText:
//
//  Error processing ID payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_ID = 13834;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_CERT
//
// MessageText:
//
//  Error processing Cert payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_CERT = 13835;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_CERT_REQ
//
// MessageText:
//
//  Error processing Certificate Request payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_CERT_REQ = 13836;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_HASH
//
// MessageText:
//
//  Error processing Hash payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_HASH = 13837;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_SIG
//
// MessageText:
//
//  Error processing Signature payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_SIG = 13838;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_NONCE
//
// MessageText:
//
//  Error processing Nonce payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_NONCE = 13839;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_NOTIFY
//
// MessageText:
//
//  Error processing Notify payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_NOTIFY = 13840;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_DELETE
//
// MessageText:
//
//  Error processing Delete Payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_DELETE = 13841;

//
// MessageId: ERROR_IPSEC_IKE_PROCESS_ERR_VENDOR
//
// MessageText:
//
//  Error processing VendorId payload
//
export const ERROR_IPSEC_IKE_PROCESS_ERR_VENDOR = 13842;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_PAYLOAD
//
// MessageText:
//
//  Invalid payload received
//
export const ERROR_IPSEC_IKE_INVALID_PAYLOAD = 13843;

//
// MessageId: ERROR_IPSEC_IKE_LOAD_SOFT_SA
//
// MessageText:
//
//  Soft SA loaded
//
export const ERROR_IPSEC_IKE_LOAD_SOFT_SA = 13844;

//
// MessageId: ERROR_IPSEC_IKE_SOFT_SA_TORN_DOWN
//
// MessageText:
//
//  Soft SA torn down
//
export const ERROR_IPSEC_IKE_SOFT_SA_TORN_DOWN = 13845;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_COOKIE
//
// MessageText:
//
//  Invalid cookie received.
//
export const ERROR_IPSEC_IKE_INVALID_COOKIE = 13846;

//
// MessageId: ERROR_IPSEC_IKE_NO_PEER_CERT
//
// MessageText:
//
//  Peer failed to send valid machine certificate
//
export const ERROR_IPSEC_IKE_NO_PEER_CERT = 13847;

//
// MessageId: ERROR_IPSEC_IKE_PEER_CRL_FAILED
//
// MessageText:
//
//  Certification Revocation check of peer's certificate failed
//
export const ERROR_IPSEC_IKE_PEER_CRL_FAILED = 13848;

//
// MessageId: ERROR_IPSEC_IKE_POLICY_CHANGE
//
// MessageText:
//
//  New policy invalidated SAs formed with old policy
//
export const ERROR_IPSEC_IKE_POLICY_CHANGE = 13849;

//
// MessageId: ERROR_IPSEC_IKE_NO_MM_POLICY
//
// MessageText:
//
//  There is no available Main Mode IKE policy.
//
export const ERROR_IPSEC_IKE_NO_MM_POLICY = 13850;

//
// MessageId: ERROR_IPSEC_IKE_NOTCBPRIV
//
// MessageText:
//
//  Failed to enabled TCB privilege.
//
export const ERROR_IPSEC_IKE_NOTCBPRIV = 13851;

//
// MessageId: ERROR_IPSEC_IKE_SECLOADFAIL
//
// MessageText:
//
//  Failed to load SECURITY.DLL.
//
export const ERROR_IPSEC_IKE_SECLOADFAIL = 13852;

//
// MessageId: ERROR_IPSEC_IKE_FAILSSPINIT
//
// MessageText:
//
//  Failed to obtain security function table dispatch address from SSPI.
//
export const ERROR_IPSEC_IKE_FAILSSPINIT = 13853;

//
// MessageId: ERROR_IPSEC_IKE_FAILQUERYSSP
//
// MessageText:
//
//  Failed to query Kerberos package to obtain max token size.
//
export const ERROR_IPSEC_IKE_FAILQUERYSSP = 13854;

//
// MessageId: ERROR_IPSEC_IKE_SRVACQFAIL
//
// MessageText:
//
//  Failed to obtain Kerberos server credentials for ISAKMP/ERROR_IPSEC_IKE service.  Kerberos authentication will not function.  The most likely reason for this is lack of domain membership.  This is normal if your computer is a member of a workgroup.
//
export const ERROR_IPSEC_IKE_SRVACQFAIL = 13855;

//
// MessageId: ERROR_IPSEC_IKE_SRVQUERYCRED
//
// MessageText:
//
//  Failed to determine SSPI principal name for ISAKMP/ERROR_IPSEC_IKE service (QueryCredentialsAttributes).
//
export const ERROR_IPSEC_IKE_SRVQUERYCRED = 13856;

//
// MessageId: ERROR_IPSEC_IKE_GETSPIFAIL
//
// MessageText:
//
//  Failed to obtain new SPI for the inbound SA from Ipsec driver.  The most common cause for this is that the driver does not have the correct filter.  Check your policy to verify the filters.
//
export const ERROR_IPSEC_IKE_GETSPIFAIL = 13857;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_FILTER
//
// MessageText:
//
//  Given filter is invalid
//
export const ERROR_IPSEC_IKE_INVALID_FILTER = 13858;

//
// MessageId: ERROR_IPSEC_IKE_OUT_OF_MEMORY
//
// MessageText:
//
//  Memory allocation failed.
//
export const ERROR_IPSEC_IKE_OUT_OF_MEMORY = 13859;

//
// MessageId: ERROR_IPSEC_IKE_ADD_UPDATE_KEY_FAILED
//
// MessageText:
//
//  Failed to add Security Association to IPSec Driver.  The most common cause for this is if the IKE negotiation took too long to complete.  If the problem persists, reduce the load on the faulting machine.
//
export const ERROR_IPSEC_IKE_ADD_UPDATE_KEY_FAILED = 13860;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_POLICY
//
// MessageText:
//
//  Invalid policy
//
export const ERROR_IPSEC_IKE_INVALID_POLICY = 13861;

//
// MessageId: ERROR_IPSEC_IKE_UNKNOWN_DOI
//
// MessageText:
//
//  Invalid DOI
//
export const ERROR_IPSEC_IKE_UNKNOWN_DOI = 13862;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_SITUATION
//
// MessageText:
//
//  Invalid situation
//
export const ERROR_IPSEC_IKE_INVALID_SITUATION = 13863;

//
// MessageId: ERROR_IPSEC_IKE_DH_FAILURE
//
// MessageText:
//
//  Diffie-Hellman failure
//
export const ERROR_IPSEC_IKE_DH_FAILURE = 13864;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_GROUP
//
// MessageText:
//
//  Invalid Diffie-Hellman group
//
export const ERROR_IPSEC_IKE_INVALID_GROUP = 13865;

//
// MessageId: ERROR_IPSEC_IKE_ENCRYPT
//
// MessageText:
//
//  Error encrypting payload
//
export const ERROR_IPSEC_IKE_ENCRYPT = 13866;

//
// MessageId: ERROR_IPSEC_IKE_DECRYPT
//
// MessageText:
//
//  Error decrypting payload
//
export const ERROR_IPSEC_IKE_DECRYPT = 13867;

//
// MessageId: ERROR_IPSEC_IKE_POLICY_MATCH
//
// MessageText:
//
//  Policy match error
//
export const ERROR_IPSEC_IKE_POLICY_MATCH = 13868;

//
// MessageId: ERROR_IPSEC_IKE_UNSUPPORTED_ID
//
// MessageText:
//
//  Unsupported ID
//
export const ERROR_IPSEC_IKE_UNSUPPORTED_ID = 13869;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_HASH
//
// MessageText:
//
//  Hash verification failed
//
export const ERROR_IPSEC_IKE_INVALID_HASH = 13870;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_HASH_ALG
//
// MessageText:
//
//  Invalid hash algorithm
//
export const ERROR_IPSEC_IKE_INVALID_HASH_ALG = 13871;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_HASH_SIZE
//
// MessageText:
//
//  Invalid hash size
//
export const ERROR_IPSEC_IKE_INVALID_HASH_SIZE = 13872;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_ENCRYPT_ALG
//
// MessageText:
//
//  Invalid encryption algorithm
//
export const ERROR_IPSEC_IKE_INVALID_ENCRYPT_ALG = 13873;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_AUTH_ALG
//
// MessageText:
//
//  Invalid authentication algorithm
//
export const ERROR_IPSEC_IKE_INVALID_AUTH_ALG = 13874;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_SIG
//
// MessageText:
//
//  Invalid certificate signature
//
export const ERROR_IPSEC_IKE_INVALID_SIG = 13875;

//
// MessageId: ERROR_IPSEC_IKE_LOAD_FAILED
//
// MessageText:
//
//  Load failed
//
export const ERROR_IPSEC_IKE_LOAD_FAILED = 13876;

//
// MessageId: ERROR_IPSEC_IKE_RPC_DELETE
//
// MessageText:
//
//  Deleted via RPC call
//
export const ERROR_IPSEC_IKE_RPC_DELETE = 13877;

//
// MessageId: ERROR_IPSEC_IKE_BENIGN_REINIT
//
// MessageText:
//
//  Temporary state created to perform reinit. This is not a real failure.
//
export const ERROR_IPSEC_IKE_BENIGN_REINIT = 13878;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_RESPONDER_LIFETIME_NOTIFY
//
// MessageText:
//
//  The lifetime value received in the Responder Lifetime Notify is below the Windows 2000 configured minimum value.  Please fix the policy on the peer machine.
//
export const ERROR_IPSEC_IKE_INVALID_RESPONDER_LIFETIME_NOTIFY = 13879;

//
// MessageId: ERROR_IPSEC_IKE_INVALID_CERT_KEYLEN
//
// MessageText:
//
//  Key length in certificate is too small for configured security requirements.
//
export const ERROR_IPSEC_IKE_INVALID_CERT_KEYLEN = 13881;

//
// MessageId: ERROR_IPSEC_IKE_MM_LIMIT
//
// MessageText:
//
//  Max number of established MM SAs to peer exceeded.
//
export const ERROR_IPSEC_IKE_MM_LIMIT = 13882;

//
// MessageId: ERROR_IPSEC_IKE_NEGOTIATION_DISABLED
//
// MessageText:
//
//  IKE received a policy that disables negotiation.
//
export const ERROR_IPSEC_IKE_NEGOTIATION_DISABLED = 13883;

//
// MessageId: ERROR_IPSEC_IKE_NEG_STATUS_END
//
// MessageText:
//
//  ERROR_IPSEC_IKE_NEG_STATUS_END
//
export const ERROR_IPSEC_IKE_NEG_STATUS_END = 13884;
