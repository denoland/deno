// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.bytecode;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassWriter;

/**
 * A ClassWriter that uses a custom class loader.
 */
class CustomClassLoaderClassWriter extends ClassWriter {
    private ClassLoader mClassLoader;

    public CustomClassLoaderClassWriter(ClassLoader classLoader, ClassReader reader, int flags) {
        super(reader, flags);
        this.mClassLoader = classLoader;
    }

    /**
     * The only modifications from the org.objectweb.asm.ClassWriter implementations is that this
     * method is final and it uses a custom ClassLoader.
     *
     * See https://github.com/llbit/ow2-asm/blob/master/src/org/objectweb/asm/ClassWriter.java.
     */
    @Override
    protected final String getCommonSuperClass(final String type1, final String type2) {
        Class<?> c, d;
        try {
            c = Class.forName(type1.replace('/', '.'), false, mClassLoader);
            d = Class.forName(type2.replace('/', '.'), false, mClassLoader);
        } catch (Exception e) {
            throw new RuntimeException(e.toString());
        }
        if (c.isAssignableFrom(d)) {
            return type1;
        }
        if (d.isAssignableFrom(c)) {
            return type2;
        }
        if (c.isInterface() || d.isInterface()) {
            return "java/lang/Object";
        } else {
            do {
                c = c.getSuperclass();
            } while (!c.isAssignableFrom(d));
            return c.getName().replace('.', '/');
        }
    }
}
