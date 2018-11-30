// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.bytecode;

import org.objectweb.asm.Type;

import java.util.HashMap;
import java.util.Map;

/**
 * Utility methods for accessing {@link Type}s Strings.
 *
 * Useful definitions to keep in mind when using this class:
 * Internal name - The fully qualified name for a type with dots replaced by slashes. Not really
 * relevant for primitive types.
 * Type descriptor - Single letters for primitive types, "L" + internal name + ";" for class types.
 *
 * The methods in this class accept internal names or primitive type descriptors.
 */
class TypeUtils {
    static final String ASSERTION_ERROR = "java/lang/AssertionError";
    static final String ASSET_MANAGER = "android/content/res/AssetManager";
    static final String BUILD_HOOKS = "org/chromium/build/BuildHooks";
    static final String BUILD_HOOKS_ANDROID = "org/chromium/build/BuildHooksAndroid";
    static final String CONFIGURATION = "android/content/res/Configuration";
    static final String CONTEXT = "android/content/Context";
    static final String CONTEXT_WRAPPER = "android/content/ContextWrapper";
    static final String DISPLAY_LEAK_ACTIVITY =
            "com/squareup/leakcanary/internal/DisplayLeakActivity";
    static final String RESOURCES = "android/content/res/Resources";
    static final String STRING = "java/lang/String";
    static final String THEME = "android/content/res/Resources$Theme";

    static final String BOOLEAN = "Z";
    static final String INT = "I";
    static final String VOID = "V";
    private static final Map<String, Type> PRIMITIVE_DESCRIPTORS;
    static {
        PRIMITIVE_DESCRIPTORS = new HashMap<>();
        PRIMITIVE_DESCRIPTORS.put(Type.BOOLEAN_TYPE.toString(), Type.BOOLEAN_TYPE);
        PRIMITIVE_DESCRIPTORS.put(Type.INT_TYPE.toString(), Type.INT_TYPE);
        PRIMITIVE_DESCRIPTORS.put(Type.VOID_TYPE.toString(), Type.VOID_TYPE);
    }

    /**
     * Returns the full method signature with internal names.
     *
     * @param methodName Name of the method (ex. "getResources").
     * @param returnType Internal name for the return type.
     * @param argumentTypes List of internal names for argument types.
     * @return String representation of the method signature.
     */
    static String getMethodSignature(
            String methodName, String returnType, String... argumentTypes) {
        return methodName + getMethodDescriptor(returnType, argumentTypes);
    }

    /**
     * Builds a method descriptor suitable for use with {@link org.objectweb.asm.MethodVisitor}.
     *
     * @param returnType Internal name for the return type of the method (primitive or class).
     * @param argumentTypes Internal names for the argument types (primitive or class).
     * @return The generated method descriptor.
     */
    static String getMethodDescriptor(String returnType, String... argumentTypes) {
        Type[] typedArguments = new Type[argumentTypes.length];
        for (int i = 0; i < argumentTypes.length; ++i) {
            // Argument list should be empty in this case, not V (void).
            assert !Type.VOID_TYPE.toString().equals(argumentTypes[i]);
            typedArguments[i] = convert(argumentTypes[i]);
        }
        return Type.getMethodDescriptor(convert(returnType), typedArguments);
    }

    /**
     * Converts an internal name for a type to a {@link Type}.
     *
     * @param type Internal name for a type (primitive or class).
     * @return The resulting Type.
     */
    private static Type convert(String type) {
        if (PRIMITIVE_DESCRIPTORS.containsKey(type)) {
            return PRIMITIVE_DESCRIPTORS.get(type);
        }
        return Type.getObjectType(type);
    }
}
