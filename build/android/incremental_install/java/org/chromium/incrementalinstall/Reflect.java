// Copyright 2015 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.incrementalinstall;

import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Arrays;

/**
 * Reflection helper methods.
 */
final class Reflect {
    /**
     * Sets the value of an object's field (even if it's not visible).
     *
     * @param instance The object containing the field to set.
     * @param name The name of the field to set.
     * @param value The new value for the field.
     */
    static void setField(Object instance, String name, Object value)
            throws ReflectiveOperationException {
        Field field = findField(instance, name);
        field.setAccessible(true);
        field.set(instance, value);
    }

    /**
     * Retrieves the value of an object's field (even if it's not visible).
     *
     * @param instance The object containing the field to set.
     * @param name The name of the field to set.
     * @return The field's value. Primitive values are returned as their boxed
     *         type.
     */
    static Object getField(Object instance, String name) throws ReflectiveOperationException {
        Field field = findField(instance, name);
        field.setAccessible(true);
        return field.get(instance);
    }

    /**
     * Concatenates two arrays into a new array. The arrays must be of the same
     * type.
     */
    static Object[] concatArrays(Object[] arrType, Object[] left, Object[] right) {
        Object[] result = (Object[]) Array.newInstance(
                arrType.getClass().getComponentType(), left.length + right.length);
        System.arraycopy(left, 0, result, 0, left.length);
        System.arraycopy(right, 0, result, left.length, right.length);
        return result;
    }

    /**
     * Invokes a method with zero or more parameters. For static methods, use the Class as the
     * instance.
     */
    static Object invokeMethod(Object instance, String name, Object... params)
            throws ReflectiveOperationException {
        boolean isStatic = instance instanceof Class;
        Class<?> clazz = isStatic ? (Class<?>) instance :  instance.getClass();
        Method method = findMethod(clazz, name, params);
        method.setAccessible(true);
        return method.invoke(instance, params);
    }

    /**
     * Calls a constructor with zero or more parameters.
     */
    static Object newInstance(Class<?> clazz, Object... params)
            throws ReflectiveOperationException {
        Constructor<?> constructor = findConstructor(clazz, params);
        constructor.setAccessible(true);
        return constructor.newInstance(params);
    }

    private static Field findField(Object instance, String name) throws NoSuchFieldException {
        boolean isStatic = instance instanceof Class;
        Class<?> clazz = isStatic ? (Class<?>) instance :  instance.getClass();
        for (; clazz != null; clazz = clazz.getSuperclass()) {
            try {
                return clazz.getDeclaredField(name);
            } catch (NoSuchFieldException e) {
                // Need to look in the super class.
            }
        }
        throw new NoSuchFieldException("Field " + name + " not found in " + instance.getClass());
    }

    private static Method findMethod(Class<?> clazz, String name, Object... params)
            throws NoSuchMethodException {
        for (; clazz != null; clazz = clazz.getSuperclass()) {
            for (Method method : clazz.getDeclaredMethods()) {
                if (method.getName().equals(name)
                        && areParametersCompatible(method.getParameterTypes(), params)) {
                    return method;
                }
            }
        }
        throw new NoSuchMethodException("Method " + name + " with parameters "
                + Arrays.asList(params) + " not found in " + clazz);
    }

    private static Constructor<?> findConstructor(Class<?> clazz, Object... params)
            throws NoSuchMethodException {
        for (Constructor<?> constructor : clazz.getDeclaredConstructors()) {
            if (areParametersCompatible(constructor.getParameterTypes(), params)) {
                return constructor;
            }
        }
        throw new NoSuchMethodException("Constructor with parameters " + Arrays.asList(params)
                + " not found in " + clazz);
    }

    private static boolean areParametersCompatible(Class<?>[] paramTypes, Object... params) {
        if (params.length != paramTypes.length) {
            return false;
        }
        for (int i = 0; i < params.length; i++) {
            if (!isAssignableFrom(paramTypes[i], params[i])) {
                return false;
            }
        }
        return true;
    }

    private static boolean isAssignableFrom(Class<?> left, Object right) {
        if (right == null) {
            return !left.isPrimitive();
        }
        Class<?> rightClazz = right.getClass();
        if (left.isPrimitive()) {
            // TODO(agrieve): Fill in the rest as needed.
            return left == boolean.class && rightClazz == Boolean.class
                   || left == int.class && rightClazz == Integer.class;
        }
        return left.isAssignableFrom(rightClazz);
    }
}
