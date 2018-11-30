# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

"""Implements commands for running and interacting with Fuchsia on QEMU."""

import boot_data
import logging
import target
import os
import platform
import socket
import subprocess
import sys
import time

from common import GetQemuRootForPlatform, EnsurePathExists


# Virtual networking configuration data for QEMU.
GUEST_NET = '192.168.3.0/24'
GUEST_IP_ADDRESS = '192.168.3.9'
HOST_IP_ADDRESS = '192.168.3.2'
GUEST_MAC_ADDRESS = '52:54:00:63:5e:7b'


def _GetAvailableTcpPort():
  """Finds a (probably) open port by opening and closing a listen socket."""
  sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
  sock.bind(("", 0))
  port = sock.getsockname()[1]
  sock.close()
  return port


class QemuTarget(target.Target):
  def __init__(self, output_dir, target_cpu, cpu_cores, system_log_file,
               ram_size_mb=2048):
    """output_dir: The directory which will contain the files that are
                   generated to support the QEMU deployment.
    target_cpu: The emulated target CPU architecture.
                Can be 'x64' or 'arm64'."""
    super(QemuTarget, self).__init__(output_dir, target_cpu)
    self._qemu_process = None
    self._ram_size_mb = ram_size_mb
    self._system_log_file = system_log_file
    self._cpu_cores = cpu_cores

  def __enter__(self):
    return self

  # Used by the context manager to ensure that QEMU is killed when the Python
  # process exits.
  def __exit__(self, exc_type, exc_val, exc_tb):
    if self._IsQemuStillRunning():
      logging.info('Shutting down QEMU.')
      self._qemu_process.kill()

  def Start(self):
    qemu_path = os.path.join(GetQemuRootForPlatform(), 'bin',
                             'qemu-system-' + self._GetTargetSdkLegacyArch())
    kernel_args = boot_data.GetKernelArgs(self._output_dir)

    # TERM=dumb tells the guest OS to not emit ANSI commands that trigger
    # noisy ANSI spew from the user's terminal emulator.
    kernel_args.append('TERM=dumb')

    # Enable logging to the serial port. This is a temporary fix to investigate
    # the root cause for https://crbug.com/869753 .
    kernel_args.append('kernel.serial=legacy')

    qemu_command = [qemu_path,
        '-m', str(self._ram_size_mb),
        '-nographic',
        '-kernel', EnsurePathExists(
            boot_data.GetTargetFile(self._GetTargetSdkArch(),
                                    'qemu-kernel.bin')),
        '-initrd', EnsurePathExists(
            boot_data.GetTargetFile(self._GetTargetSdkArch(),
                                    'fuchsia.zbi')),
        '-smp', str(self._cpu_cores),

        # Attach the blobstore and data volumes. Use snapshot mode to discard
        # any changes.
        '-snapshot',
        '-drive', 'file=%s,format=qcow2,if=none,id=data,snapshot=on' %
                    EnsurePathExists(os.path.join(self._output_dir,
                                                  'fvm.blk.qcow2')),
        '-drive', 'file=%s,format=qcow2,if=none,id=blobstore,snapshot=on' %
            EnsurePathExists(
                boot_data.ConfigureDataFVM(self._output_dir,
                                           boot_data.FVM_TYPE_QCOW)),
        '-device', 'virtio-blk-pci,drive=data',
        '-device', 'virtio-blk-pci,drive=blobstore',

        # Use stdio for the guest OS only; don't attach the QEMU interactive
        # monitor.
        '-serial', 'stdio',
        '-monitor', 'none',

        '-append', ' '.join(kernel_args)
      ]

    # Configure the machine & CPU to emulate, based on the target architecture.
    if self._target_cpu == 'arm64':
      qemu_command.extend([
          '-machine','virt',
          '-cpu', 'cortex-a53',
      ])
      netdev_type = 'virtio-net-pci'
    else:
      qemu_command.extend([
          '-machine', 'q35',
      ])
      netdev_type = 'e1000'

    # On Linux, enable lightweight virtualization (KVM) if the host and guest
    # architectures are the same.
    if sys.platform.startswith('linux'):
      if self._target_cpu == 'arm64' and platform.machine() == 'aarch64':
        qemu_command.append('-enable-kvm')
      elif self._target_cpu == 'x64' and platform.machine() == 'x86_64':
        qemu_command.extend([
            '-enable-kvm', '-cpu', 'host,migratable=no',
        ])

    # Configure virtual network. It is used in the tests to connect to
    # testserver running on the host.
    netdev_config = 'user,id=net0,net=%s,dhcpstart=%s,host=%s' % \
            (GUEST_NET, GUEST_IP_ADDRESS, HOST_IP_ADDRESS)

    self._host_ssh_port = _GetAvailableTcpPort()
    netdev_config += ",hostfwd=tcp::%s-:22" % self._host_ssh_port
    qemu_command.extend([
      '-netdev', netdev_config,
      '-device', '%s,netdev=net0,mac=%s' % (netdev_type, GUEST_MAC_ADDRESS),
    ])

    # We pass a separate stdin stream to qemu. Sharing stdin across processes
    # leads to flakiness due to the OS prematurely killing the stream and the
    # Python script panicking and aborting.
    # The precise root cause is still nebulous, but this fix works.
    # See crbug.com/741194.
    logging.debug('Launching QEMU.')
    logging.debug(' '.join(qemu_command))

    # Zircon sends debug logs to serial port (see kernel.serial=legacy flag
    # above). Serial port is redirected to a file through QEMU stdout.
    # This approach is used instead of loglistener to debug
    # https://crbug.com/86975 .
    if self._system_log_file:
      stdout = self._system_log_file
      stderr = subprocess.STDOUT
    else:
      stdout = open(os.devnull)
      stderr = sys.stderr

    self._qemu_process = subprocess.Popen(qemu_command, stdin=open(os.devnull),
                                          stdout=stdout, stderr=stderr)
    self._WaitUntilReady();

  def _IsQemuStillRunning(self):
    return os.waitpid(self._qemu_process.pid, os.WNOHANG)[0] == 0

  def _GetEndpoint(self):
    if not self._IsQemuStillRunning():
      raise Exception('QEMU quit unexpectedly.')
    return ('localhost', self._host_ssh_port)

  def _GetSshConfigPath(self):
    return boot_data.GetSSHConfigPath(self._output_dir)
