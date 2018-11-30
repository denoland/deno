# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import unittest
import compile_xcassets


class TestFilterCompilerOutput(unittest.TestCase):

  relative_paths = {
    '/Users/janedoe/chromium/src/Chromium.xcassets':
        '../../Chromium.xcassets',
    '/Users/janedoe/chromium/src/out/Default/Chromium.app/Assets.car':
        'Chromium.app/Assets.car',
  }

  def testNoError(self):
    self.assertEquals(
        '',
        compile_xcassets.FilterCompilerOutput(
            '/* com.apple.actool.compilation-results */\n'
            '/Users/janedoe/chromium/src/out/Default/Chromium.app/Assets.car\n',
            self.relative_paths))

  def testNoErrorRandomMessages(self):
    self.assertEquals(
        '',
        compile_xcassets.FilterCompilerOutput(
            '2017-07-04 04:59:19.460 ibtoold[23487:41214] CoreSimulator is att'
                'empting to unload a stale CoreSimulatorService job.  Existing'
                ' job (com.apple.CoreSimulator.CoreSimulatorService.179.1.E8tt'
                'yeDeVgWK) is from an older version and is being removed to pr'
                'event problems.\n'
            '/* com.apple.actool.compilation-results */\n'
            '/Users/janedoe/chromium/src/out/Default/Chromium.app/Assets.car\n',
            self.relative_paths))

  def testWarning(self):
    self.assertEquals(
        '/* com.apple.actool.document.warnings */\n'
        '../../Chromium.xcassets:./image1.imageset/[universal][][][1x][][][]['
            '][][]: warning: The file "image1.png" for the image set "image1"'
            ' does not exist.\n',
        compile_xcassets.FilterCompilerOutput(
            '/* com.apple.actool.document.warnings */\n'
            '/Users/janedoe/chromium/src/Chromium.xcassets:./image1.imageset/['
                'universal][][][1x][][][][][][]: warning: The file "image1.png'
                '" for the image set "image1" does not exist.\n'
            '/* com.apple.actool.compilation-results */\n'
            '/Users/janedoe/chromium/src/out/Default/Chromium.app/Assets.car\n',
            self.relative_paths))

  def testError(self):
    self.assertEquals(
        '/* com.apple.actool.errors */\n'
        '../../Chromium.xcassets: error: The output directory "/Users/janedoe/'
            'chromium/src/out/Default/Chromium.app" does not exist.\n',
        compile_xcassets.FilterCompilerOutput(
            '/* com.apple.actool.errors */\n'
            '/Users/janedoe/chromium/src/Chromium.xcassets: error: The output '
                'directory "/Users/janedoe/chromium/src/out/Default/Chromium.a'
                'pp" does not exist.\n'
            '/* com.apple.actool.compilation-results */\n',
            self.relative_paths))

  def testSpurious(self):
    self.assertEquals(
        '/* com.apple.actool.document.warnings */\n'
        '../../Chromium.xcassets:./AppIcon.appiconset: warning: A 1024x1024 ap'
            'p store icon is required for iOS apps\n',
        compile_xcassets.FilterCompilerOutput(
            '/* com.apple.actool.document.warnings */\n'
            '/Users/janedoe/chromium/src/Chromium.xcassets:./AppIcon.appiconse'
                't: warning: A 1024x1024 app store icon is required for iOS ap'
                'ps\n'
            '/* com.apple.actool.document.notices */\n'
            '/Users/janedoe/chromium/src/Chromium.xcassets:./AppIcon.appiconse'
                't/[][ipad][76x76][][][1x][][]: notice: (null)\n',
            self.relative_paths))

  def testComplexError(self):
    self.assertEquals(
        '/* com.apple.actool.errors */\n'
        ': error: Failed to find a suitable device for the type SimDeviceType '
            ': com.apple.dt.Xcode.IBSimDeviceType.iPad-2x with runtime SimRunt'
            'ime : 10.3.1 (14E8301) - com.apple.CoreSimulator.SimRuntime.iOS-1'
            '0-3\n'
        '    Failure Reason: Failed to create SimDeviceSet at path /Users/jane'
            'doe/Library/Developer/Xcode/UserData/IB Support/Simulator Devices'
            '. You\'ll want to check the logs in ~/Library/Logs/CoreSimulator '
            'to see why creating the SimDeviceSet failed.\n'
        '    Underlying Errors:\n'
        '        Description: Failed to initialize simulator device set.\n'
        '        Failure Reason: Failed to subscribe to notifications from Cor'
            'eSimulatorService.\n'
        '        Underlying Errors:\n'
        '            Description: Error returned in reply to notification requ'
            'est: Connection invalid\n'
        '            Failure Reason: Software caused connection abort\n',
        compile_xcassets.FilterCompilerOutput(
            '2017-07-07 10:37:27.367 ibtoold[88538:12553239] CoreSimulator det'
                'ected Xcode.app relocation or CoreSimulatorService version ch'
                'ange.  Framework path (/Applications/Xcode.app/Contents/Devel'
                'oper/Library/PrivateFrameworks/CoreSimulator.framework) and v'
                'ersion (375.21) does not match existing job path (/Library/De'
                'veloper/PrivateFrameworks/CoreSimulator.framework/Versions/A/'
                'XPCServices/com.apple.CoreSimulator.CoreSimulatorService.xpc)'
                ' and version (459.13).  Attempting to remove the stale servic'
                'e in order to add the expected version.\n'
            '2017-07-07 10:37:27.625 ibtoold[88538:12553256] CoreSimulatorServ'
                'ice connection interrupted.  Resubscribing to notifications.\n'
            '2017-07-07 10:37:27.632 ibtoold[88538:12553264] CoreSimulatorServ'
                'ice connection became invalid.  Simulator services will no lo'
                'nger be available.\n'
            '2017-07-07 10:37:27.642 ibtoold[88538:12553274] CoreSimulatorServ'
                'ice connection became invalid.  Simulator services will no lo'
                'nger be available.\n'
            '/* com.apple.actool.errors */\n'
            ': error: Failed to find a suitable device for the type SimDeviceT'
                'ype : com.apple.dt.Xcode.IBSimDeviceType.iPad-2x with runtime'
                ' SimRuntime : 10.3.1 (14E8301) - com.apple.CoreSimulator.SimR'
                'untime.iOS-10-3\n'
            '    Failure Reason: Failed to create SimDeviceSet at path /Users/'
                'janedoe/Library/Developer/Xcode/UserData/IB Support/Simulator'
                ' Devices. You\'ll want to check the logs in ~/Library/Logs/Co'
                'reSimulator to see why creating the SimDeviceSet failed.\n'
            '    Underlying Errors:\n'
            '        Description: Failed to initialize simulator device set.\n'
            '        Failure Reason: Failed to subscribe to notifications from'
                ' CoreSimulatorService.\n'
            '        Underlying Errors:\n'
            '            Description: Error returned in reply to notification '
                'request: Connection invalid\n'
            '            Failure Reason: Software caused connection abort\n'
            '/* com.apple.actool.compilation-results */\n',
            self.relative_paths))


if __name__ == '__main__':
  unittest.main()
