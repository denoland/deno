// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.bytecode;

import static org.objectweb.asm.Opcodes.ACC_PUBLIC;
import static org.objectweb.asm.Opcodes.ACONST_NULL;
import static org.objectweb.asm.Opcodes.ALOAD;
import static org.objectweb.asm.Opcodes.ARETURN;
import static org.objectweb.asm.Opcodes.ASM5;
import static org.objectweb.asm.Opcodes.BIPUSH;
import static org.objectweb.asm.Opcodes.GETSTATIC;
import static org.objectweb.asm.Opcodes.IFNE;
import static org.objectweb.asm.Opcodes.IF_ICMPGE;
import static org.objectweb.asm.Opcodes.ILOAD;
import static org.objectweb.asm.Opcodes.INVOKESPECIAL;
import static org.objectweb.asm.Opcodes.INVOKESTATIC;
import static org.objectweb.asm.Opcodes.INVOKEVIRTUAL;
import static org.objectweb.asm.Opcodes.RETURN;

import static org.chromium.bytecode.TypeUtils.ASSET_MANAGER;
import static org.chromium.bytecode.TypeUtils.BOOLEAN;
import static org.chromium.bytecode.TypeUtils.BUILD_HOOKS_ANDROID;
import static org.chromium.bytecode.TypeUtils.CONFIGURATION;
import static org.chromium.bytecode.TypeUtils.CONTEXT;
import static org.chromium.bytecode.TypeUtils.CONTEXT_WRAPPER;
import static org.chromium.bytecode.TypeUtils.DISPLAY_LEAK_ACTIVITY;
import static org.chromium.bytecode.TypeUtils.INT;
import static org.chromium.bytecode.TypeUtils.RESOURCES;
import static org.chromium.bytecode.TypeUtils.STRING;
import static org.chromium.bytecode.TypeUtils.THEME;
import static org.chromium.bytecode.TypeUtils.VOID;

import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.Label;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import java.util.Arrays;
import java.util.List;

/**
 * A ClassVisitor for providing access to custom resources via BuildHooksAndroid.
 *
 * The goal of this class is to provide hooks into all places where android resources
 * are available so that they can be modified before use. This is done by rewriting the bytecode
 * for all callable definitions of certain Context methods, specifically:
 * - getResources
 * - getAssets
 * - getTheme
 * - setTheme
 * - createConfigurationContext
 *
 * Only classes at the framework boundary are rewritten since presumably all other indirect Context
 * subclasses will end up calling their respective super methods (i.e. we bytecode rewrite
 * BaseChromiumApplication since it extends Application, but not ContentApplication since it
 * extends a non-framework subclass.
 */
class CustomResourcesClassAdapter extends ClassVisitor {
    private static final String IS_ENABLED_METHOD = "isEnabled";
    private static final String IS_ENABLED_DESCRIPTOR = TypeUtils.getMethodDescriptor(BOOLEAN);
    // Cached since this is used so often.
    private static final String GET_IDENTIFIER_DESCRIPTOR =
            TypeUtils.getMethodDescriptor(INT, STRING, STRING, STRING);

    // Existing methods are more difficult to handle, and not currently needed.
    private static final List<String> PROHIBITED_METHODS = Arrays.asList(
            TypeUtils.getMethodSignature("getResources", RESOURCES),
            TypeUtils.getMethodSignature("getAssets", ASSET_MANAGER),
            TypeUtils.getMethodSignature("getTheme", THEME),
            TypeUtils.getMethodSignature("createConfigurationContext", CONTEXT, CONFIGURATION),
            TypeUtils.getMethodSignature("setTheme", VOID, INT));

    private static final List<String> EXCEPTED_CLASS_METHODS = Arrays.asList(
            DISPLAY_LEAK_ACTIVITY + TypeUtils.getMethodSignature("setTheme", VOID, INT));

    private boolean mShouldTransform;
    private String mClassName;
    private String mSuperClassName;
    private ClassLoader mClassLoader;

    CustomResourcesClassAdapter(ClassVisitor visitor, String className, String superClassName,
            ClassLoader classLoader) {
        super(ASM5, visitor);
        this.mClassName = className;
        this.mSuperClassName = superClassName;
        this.mClassLoader = classLoader;
    }

    @Override
    public void visit(int version, int access, String name, String signature, String superName,
            String[] interfaces) {
        super.visit(version, access, name, signature, superName, interfaces);
        mShouldTransform = shouldTransform();
    }

    @Override
    public MethodVisitor visitMethod(final int access, final String name, String desc,
            String signature, String[] exceptions) {
        if (mShouldTransform) {
            String methodSignature = name + desc;
            if (requiresModifyingExisting(methodSignature)) {
                throw new RuntimeException("Rewriting existing methods not supported: " + mClassName
                        + "#" + methodSignature);
            }
        }
        return new RewriteGetIdentifierMethodVisitor(
                super.visitMethod(access, name, desc, signature, exceptions));
    }

    @Override
    public void visitEnd() {
        if (mShouldTransform) {
            delegateCreateConfigurationContext();
            delegateSetTheme();
            delegateGet("getAssets", ASSET_MANAGER);
            delegateGet("getTheme", THEME);
            delegateGet("getResources", RESOURCES);
        }
        super.visitEnd();
    }

    private boolean requiresModifyingExisting(String methodDescriptor) {
        // TODO(estevenson): Remove LeakCanary special casing once we get rid of LeakCanary.
        return PROHIBITED_METHODS.contains(methodDescriptor)
                && !EXCEPTED_CLASS_METHODS.contains(mClassName + methodDescriptor);
    }

    private boolean shouldTransform() {
        if (!isDescendantOfContext()) {
            return false;
        }
        if (!superClassIsFrameworkClass()) {
            return false;
        }
        return !superClassIsContextWrapper();
    }

    private boolean superClassIsFrameworkClass() {
        return loadClass(mSuperClassName).getProtectionDomain().toString().contains("android.jar");
    }

    private boolean isDescendantOfContext() {
        return isSubClass(mClassName, CONTEXT);
    }

    private boolean superClassIsContextWrapper() {
        return mSuperClassName.equals(CONTEXT_WRAPPER);
    }

    private boolean isSubClass(String candidate, String other) {
        Class<?> candidateClazz = loadClass(candidate);
        Class<?> parentClazz = loadClass(other);
        return parentClazz.isAssignableFrom(candidateClazz);
    }

    private Class<?> loadClass(String className) {
        try {
            return mClassLoader.loadClass(className.replace('/', '.'));
        } catch (ClassNotFoundException e) {
            throw new RuntimeException(e);
        }
    }

    /**
     * Remaps Resources.getIdentifier() method calls to use BuildHooksAndroid.
     *
     * resourceObj.getIdentifier(String, String, String) becomes:
     * BuildHooksAndroid.getIdentifier(resourceObj, String, String, String);
     */
    private static final class RewriteGetIdentifierMethodVisitor extends MethodVisitor {
        RewriteGetIdentifierMethodVisitor(MethodVisitor mv) {
            super(ASM5, mv);
        }

        @Override
        public void visitMethodInsn(
                int opcode, String owner, String name, String desc, boolean itf) {
            String methodName = "getIdentifier";
            if (opcode == INVOKEVIRTUAL && owner.equals(RESOURCES) && name.equals(methodName)
                    && desc.equals(GET_IDENTIFIER_DESCRIPTOR)) {
                super.visitMethodInsn(INVOKESTATIC, BUILD_HOOKS_ANDROID, methodName,
                        TypeUtils.getMethodDescriptor(INT, RESOURCES, STRING, STRING, STRING), itf);
            } else {
                super.visitMethodInsn(opcode, owner, name, desc, itf);
            }
        }
    }

    /**
     * Generates:
     *
     * <pre>
     * public Context createConfigurationContext(Configuration configuration) {
     *   // createConfigurationContext does not exist before API level 17.
     *   if (Build.VERSION.SDK_INT < 17) return null;
     *   if (!BuildHooksAndroid.isEnabled()) return super.createConfigurationContext(configuration);
     *   return BuildHooksAndroid.createConfigurationContext(
     *          super.createConfigurationContext(configuration));
     * }
     * </pre>
     * }
     */
    private void delegateCreateConfigurationContext() {
        String methodName = "createConfigurationContext";
        String methodDescriptor = TypeUtils.getMethodDescriptor(CONTEXT, CONFIGURATION);
        MethodVisitor mv = super.visitMethod(ACC_PUBLIC, methodName, methodDescriptor, null, null);
        mv.visitCode();
        mv.visitFieldInsn(GETSTATIC, "android/os/Build$VERSION", "SDK_INT", INT);
        mv.visitIntInsn(BIPUSH, 17);
        Label l0 = new Label();
        mv.visitJumpInsn(IF_ICMPGE, l0);
        mv.visitInsn(ACONST_NULL);
        mv.visitInsn(ARETURN);
        mv.visitLabel(l0);
        mv.visitFrame(Opcodes.F_SAME, 0, null, 0, null);
        mv.visitMethodInsn(
                INVOKESTATIC, BUILD_HOOKS_ANDROID, IS_ENABLED_METHOD, IS_ENABLED_DESCRIPTOR, false);
        Label l1 = new Label();
        mv.visitJumpInsn(IFNE, l1);
        mv.visitVarInsn(ALOAD, 0);
        mv.visitVarInsn(ALOAD, 1);
        mv.visitMethodInsn(INVOKESPECIAL, mSuperClassName, methodName, methodDescriptor, false);
        mv.visitInsn(ARETURN);
        mv.visitLabel(l1);
        mv.visitFrame(Opcodes.F_SAME, 0, null, 0, null);
        mv.visitVarInsn(ALOAD, 0);
        mv.visitVarInsn(ALOAD, 1);
        mv.visitMethodInsn(INVOKESPECIAL, mSuperClassName, methodName, methodDescriptor, false);
        mv.visitMethodInsn(INVOKESTATIC, BUILD_HOOKS_ANDROID, methodName,
                TypeUtils.getMethodDescriptor(CONTEXT, CONTEXT), false);
        mv.visitInsn(ARETURN);
        mv.visitMaxs(2, 2);
        mv.visitEnd();
    }

    /**
     * Generates:
     *
     * <pre>
     * public void setTheme(int theme) {
     *   if (!BuildHooksAndroid.isEnabled()) {
     *     super.setTheme(theme);
     *     return;
     *   }
     *   BuildHooksAndroid.setTheme(this, theme);
     * }
     * </pre>
     */
    private void delegateSetTheme() {
        String methodName = "setTheme";
        String methodDescriptor = TypeUtils.getMethodDescriptor(VOID, INT);
        String buildHooksMethodDescriptor = TypeUtils.getMethodDescriptor(VOID, CONTEXT, INT);
        MethodVisitor mv = super.visitMethod(ACC_PUBLIC, methodName, methodDescriptor, null, null);
        mv.visitCode();
        mv.visitMethodInsn(
                INVOKESTATIC, BUILD_HOOKS_ANDROID, IS_ENABLED_METHOD, IS_ENABLED_DESCRIPTOR, false);
        Label l0 = new Label();
        mv.visitJumpInsn(IFNE, l0);
        mv.visitVarInsn(ALOAD, 0);
        mv.visitVarInsn(ILOAD, 1);
        mv.visitMethodInsn(INVOKESPECIAL, mSuperClassName, methodName, methodDescriptor, false);
        mv.visitInsn(RETURN);
        mv.visitLabel(l0);
        mv.visitFrame(Opcodes.F_SAME, 0, null, 0, null);
        mv.visitVarInsn(ALOAD, 0);
        mv.visitVarInsn(ILOAD, 1);
        mv.visitMethodInsn(
                INVOKESTATIC, BUILD_HOOKS_ANDROID, methodName, buildHooksMethodDescriptor, false);
        mv.visitInsn(RETURN);
        mv.visitMaxs(2, 2);
        mv.visitEnd();
    }

    /**
     * Generates:
     *
     * <pre>
     * public returnType methodName() {
     *   if (!BuildHooksAndroid.isEnabled()) return super.methodName();
     *   return BuildHooksAndroid.methodName(this);
     * }
     * </pre>
     */
    private void delegateGet(String methodName, String returnType) {
        String getMethodDescriptor = TypeUtils.getMethodDescriptor(returnType);
        String buildHooksGetMethodDescriptor = TypeUtils.getMethodDescriptor(returnType, CONTEXT);
        MethodVisitor mv =
                super.visitMethod(ACC_PUBLIC, methodName, getMethodDescriptor, null, null);
        mv.visitCode();
        mv.visitMethodInsn(
                INVOKESTATIC, BUILD_HOOKS_ANDROID, IS_ENABLED_METHOD, IS_ENABLED_DESCRIPTOR, false);
        Label l0 = new Label();
        mv.visitJumpInsn(IFNE, l0);
        mv.visitVarInsn(ALOAD, 0);
        mv.visitMethodInsn(INVOKESPECIAL, mSuperClassName, methodName, getMethodDescriptor, false);
        mv.visitInsn(ARETURN);
        mv.visitLabel(l0);
        mv.visitFrame(Opcodes.F_SAME, 0, null, 0, null);
        mv.visitVarInsn(ALOAD, 0);
        mv.visitMethodInsn(INVOKESTATIC, BUILD_HOOKS_ANDROID, methodName,
                buildHooksGetMethodDescriptor, false);
        mv.visitInsn(ARETURN);
        mv.visitMaxs(1, 1);
        mv.visitEnd();
    }
}
