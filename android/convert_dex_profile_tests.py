# Copyright 2018 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.

import unittest
import tempfile
import convert_dex_profile as cp

cp.logging.disable(cp.logging.CRITICAL)

DEX_DUMP = """

Class descriptor  : 'La;'
  Direct methods    -
      #0              : (in La;)
        name          : '<clinit>'
        type          : '(Ljava/lang/String;)V'
        code          -
        catches       : 1
                0x000f - 0x001e
                  <any> -> 0x0093
        positions     :
                0x0001 line=310
                0x0057 line=313
        locals        : 
      #1              : (in La;)
        name          : '<init>'
        type          : '()V'
        positions     :
        locals        : 
  Virtual methods   -
      #0              : (in La;)
        name          : 'a'
        type          : '(Ljava/lang/String;)I'
        positions     : 
          0x0000 line=2
          0x0003 line=3
          0x001b line=8
        locals        : 
          0x0000 - 0x0021 reg=3 this La; 
      #1              : (in La;)
        name          : 'a'
        type          : '(Ljava/lang/Object;)I'
        positions     : 
          0x0000 line=8
          0x0003 line=9
        locals        : 
          0x0000 - 0x0021 reg=3 this La; 
      #2              : (in La;)
        name          : 'b'
        type          : '()La;'
        positions     : 
          0x0000 line=1
        locals        : 
"""

# pylint: disable=line-too-long
PROGUARD_MAPPING = \
"""org.chromium.Original -> a:
    org.chromium.Original sDisplayAndroidManager -> e
    org.chromium.Original another() -> b
    4:4:void inlined():237:237 -> a
    4:4:org.chromium.Original getInstance():203 -> a
    5:5:void org.chromium.Original$Subclass.<init>(org.chromium.Original,byte):130:130 -> a
    5:5:void initialize():237 -> a
    5:5:org.chromium.Original getInstance():203 -> a
    6:6:void initialize():237:237 -> a
    9:9:android.content.Context org.chromium.base.ContextUtils.getApplicationContext():49:49 -> a
    9:9:android.content.Context getContext():219 -> a
    9:9:void initialize():245 -> a
    9:9:org.chromium.Original getInstance():203 -> a"""

OBFUSCATED_PROFILE = \
"""La;
PLa;->b()La;
SLa;->a(Ljava/lang/Object;)I
HPLa;->a(Ljava/lang/String;)I"""

UNOBFUSCATED_PROFILE = \
"""Lorg/chromium/Original;
PLorg/chromium/Original;->another()Lorg/chromium/Original;
HPSLorg/chromium/Original;->getInstance()Lorg/chromium/Original;
HPLorg/chromium/Original;->initialize()V"""

class GenerateProfileTests(unittest.TestCase):
  def testProcessDex(self):
    dex = cp.ProcessDex(DEX_DUMP.splitlines())
    self.assertIsNotNone(dex['a'])

    self.assertEquals(len(dex['a'].FindMethodsAtLine('<clinit>', 311, 313)), 1)
    self.assertEquals(len(dex['a'].FindMethodsAtLine('<clinit>', 309, 315)), 1)
    clinit = dex['a'].FindMethodsAtLine('<clinit>', 311, 313)[0]
    self.assertEquals(clinit.name, '<clinit>')
    self.assertEquals(clinit.return_type, 'V')
    self.assertEquals(clinit.param_types, 'Ljava/lang/String;')

    self.assertEquals(len(dex['a'].FindMethodsAtLine('a', 8, None)), 2)
    self.assertIsNone(dex['a'].FindMethodsAtLine('a', 100, None))

# pylint: disable=protected-access
  def testProcessProguardMapping(self):
    dex = cp.ProcessDex(DEX_DUMP.splitlines())
    mapping, reverse = cp.ProcessProguardMapping(
        PROGUARD_MAPPING.splitlines(), dex)

    self.assertEquals('La;', reverse.GetClassMapping('Lorg/chromium/Original;'))

    getInstance = cp.Method(
        'getInstance', 'Lorg/chromium/Original;', '', 'Lorg/chromium/Original;')
    initialize = cp.Method('initialize', 'Lorg/chromium/Original;', '', 'V')
    another = cp.Method(
        'another', 'Lorg/chromium/Original;', '', 'Lorg/chromium/Original;')
    subclassInit = cp.Method(
        '<init>', 'Lorg/chromium/Original$Subclass;',
        'Lorg/chromium/Original;B', 'V')

    mapped = mapping.GetMethodMapping(
        cp.Method('a', 'La;', 'Ljava/lang/String;', 'I'))
    self.assertEquals(len(mapped), 2)
    self.assertIn(getInstance, mapped)
    self.assertNotIn(subclassInit, mapped)
    self.assertNotIn(
        cp.Method('inlined', 'Lorg/chromium/Original;', '', 'V'), mapped)
    self.assertIn(initialize, mapped)

    mapped = mapping.GetMethodMapping(
        cp.Method('a', 'La;', 'Ljava/lang/Object;', 'I'))
    self.assertEquals(len(mapped), 1)
    self.assertIn(getInstance, mapped)

    mapped = mapping.GetMethodMapping(cp.Method('b', 'La;', '', 'La;'))
    self.assertEquals(len(mapped), 1)
    self.assertIn(another, mapped)

    for from_method, to_methods in mapping._method_mapping.iteritems():
      for to_method in to_methods:
        self.assertIn(from_method, reverse.GetMethodMapping(to_method))
    for from_class, to_class in mapping._class_mapping.iteritems():
      self.assertEquals(from_class, reverse.GetClassMapping(to_class))

  def testProcessProfile(self):
    dex = cp.ProcessDex(DEX_DUMP.splitlines())
    mapping, _ = cp.ProcessProguardMapping(PROGUARD_MAPPING.splitlines(), dex)
    profile = cp.ProcessProfile(OBFUSCATED_PROFILE.splitlines(), mapping)

    getInstance = cp.Method(
        'getInstance', 'Lorg/chromium/Original;', '', 'Lorg/chromium/Original;')
    initialize = cp.Method('initialize', 'Lorg/chromium/Original;', '', 'V')
    another = cp.Method(
        'another', 'Lorg/chromium/Original;', '', 'Lorg/chromium/Original;')

    self.assertIn('Lorg/chromium/Original;', profile._classes)
    self.assertIn(getInstance, profile._methods)
    self.assertIn(initialize, profile._methods)
    self.assertIn(another, profile._methods)

    self.assertEquals(profile._methods[getInstance], set(['H', 'S', 'P']))
    self.assertEquals(profile._methods[initialize], set(['H', 'P']))
    self.assertEquals(profile._methods[another], set(['P']))

  def testEndToEnd(self):
    dex = cp.ProcessDex(DEX_DUMP.splitlines())
    mapping, _ = cp.ProcessProguardMapping(PROGUARD_MAPPING.splitlines(), dex)

    profile = cp.ProcessProfile(OBFUSCATED_PROFILE.splitlines(), mapping)
    with tempfile.NamedTemporaryFile() as temp:
      profile.WriteToFile(temp.name)
      with open(temp.name, 'r') as f:
        for a, b in zip(sorted(f), sorted(UNOBFUSCATED_PROFILE.splitlines())):
          self.assertEquals(a.strip(), b.strip())


if __name__ == '__main__':
  unittest.main()
