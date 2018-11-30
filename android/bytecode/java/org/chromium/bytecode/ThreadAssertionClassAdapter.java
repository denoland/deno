// Copyright 2018 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.bytecode;

import static org.objectweb.asm.Opcodes.ASM5;
import static org.objectweb.asm.Opcodes.INVOKESTATIC;

import org.objectweb.asm.AnnotationVisitor;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.MethodVisitor;

/**
 * A ClassVisitor which adds calls to
 * {@link org.chromium.base.ThreadUtils}'s assertOnUiThread/assertOnBackgroundThread when the
 * corresponding {@link android.support.annotation.UiThread} or
 * {@link android.support.annotation.WorkerThread} annotations are present. The function calls
 * are placed at the start of the method.
 */
class ThreadAssertionClassAdapter extends ClassVisitor {
    private static final String THREAD_UTILS_DESCRIPTOR = "org/chromium/base/ThreadUtils";
    private static final String THREAD_UTILS_SIGNATURE = "()V";
    private static final String UI_THREAD_ANNOTATION_DESCRIPTOR =
            "Landroid/support/annotation/UiThread;";
    private static final String WORKER_THREAD_ANNOTATION_DESCRIPTOR =
            "Landroid/support/annotation/WorkerThread;";

    ThreadAssertionClassAdapter(ClassVisitor visitor) {
        super(ASM5, visitor);
    }

    @Override
    public MethodVisitor visitMethod(final int access, final String name, String desc,
            String signature, String[] exceptions) {
        return new AddAssertMethodVisitor(
                super.visitMethod(access, name, desc, signature, exceptions));
    }

    private static class AddAssertMethodVisitor extends MethodVisitor {
        String mAssertMethodName = "";

        AddAssertMethodVisitor(MethodVisitor mv) {
            super(ASM5, mv);
        }

        /**
         * Call for annotations on the method. Checks if the annotation is @UiThread
         * or @WorkerThread, and if so will set the mAssertMethodName property to the name of the
         * method to call in order to assert that a method is running on the intented thread.
         *
         * @param descriptor Annotation descriptor containing its name and package.
         */
        @Override
        public AnnotationVisitor visitAnnotation(String descriptor, boolean visible) {
            switch (descriptor) {
                case UI_THREAD_ANNOTATION_DESCRIPTOR:
                    mAssertMethodName = "assertOnUiThread";
                    break;
                case WORKER_THREAD_ANNOTATION_DESCRIPTOR:
                    mAssertMethodName = "assertOnBackgroundThread";
                    break;
                default:
                    break;
            }

            return super.visitAnnotation(descriptor, visible);
        }

        /**
         * Called to start visiting code. Will also insert the assertOnXThread methods at the start
         * of the method if needed.
         */
        @Override
        public void visitCode() {
            super.visitCode();
            if (!mAssertMethodName.equals("")) {
                visitMethodInsn(INVOKESTATIC, THREAD_UTILS_DESCRIPTOR, mAssertMethodName,
                        THREAD_UTILS_SIGNATURE, false);
            }
        }
    }
}