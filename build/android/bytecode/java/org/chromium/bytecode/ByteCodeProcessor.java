// Copyright 2017 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

package org.chromium.bytecode;

import static org.objectweb.asm.ClassWriter.COMPUTE_FRAMES;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.MalformedURLException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.zip.CRC32;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;

/**
 * Java application that takes in an input jar, performs a series of bytecode transformations,
 * and generates an output jar.
 *
 * Two types of transformations are performed:
 * 1) Enabling assertions via {@link AssertionEnablerClassAdapter}
 * 2) Providing support for custom resources via {@link CustomResourcesClassAdapter}
 */
class ByteCodeProcessor {
    private static final String CLASS_FILE_SUFFIX = ".class";
    private static final String TEMPORARY_FILE_SUFFIX = ".temp";
    private static final int BUFFER_SIZE = 16384;

    private static void writeZipEntry(ZipOutputStream zipStream, String zipPath, byte[] data)
            throws IOException {
        ZipEntry entry = new ZipEntry(zipPath);
        entry.setMethod(ZipEntry.STORED);
        entry.setTime(0);
        entry.setSize(data.length);
        CRC32 crc = new CRC32();
        crc.update(data);
        entry.setCrc(crc.getValue());
        zipStream.putNextEntry(entry);
        zipStream.write(data);
        zipStream.closeEntry();
    }

    private static void process(String inputJarPath, String outputJarPath, boolean shouldAssert,
            boolean shouldUseCustomResources, boolean shouldUseThreadAnnotations,
            ClassLoader classPathJarsClassLoader) {
        String tempJarPath = outputJarPath + TEMPORARY_FILE_SUFFIX;
        try (ZipInputStream inputStream = new ZipInputStream(
                     new BufferedInputStream(new FileInputStream(inputJarPath)));
                ZipOutputStream tempStream = new ZipOutputStream(
                        new BufferedOutputStream(new FileOutputStream(tempJarPath)))) {
            ZipEntry entry;

            while ((entry = inputStream.getNextEntry()) != null) {
                // Copy all non-.class files to the output jar.
                if (entry.isDirectory() || !entry.getName().endsWith(CLASS_FILE_SUFFIX)) {
                    tempStream.putNextEntry(entry);
                    tempStream.write(readAllBytes(inputStream));
                    tempStream.closeEntry();
                    continue;
                }

                ClassReader reader = new ClassReader(readAllBytes(inputStream));

                ClassWriter writer;
                if (shouldUseCustomResources) {
                    // Use the COMPUTE_FRAMES flag to have asm figure out the stack map frames.
                    // This is necessary because GCMBaseIntentService in android_gcm_java contains
                    // incorrect stack map frames. This option slows down processing time by 2x.
                    writer = new CustomClassLoaderClassWriter(
                            classPathJarsClassLoader, reader, COMPUTE_FRAMES);
                } else {
                    writer = new ClassWriter(reader, 0);
                }
                ClassVisitor chain = writer;
                /* DEBUGGING:
                 To see the bytecode for a specific class:
                   if (entry.getName().contains("YourClassName")) {
                     chain = new TraceClassVisitor(chain, new PrintWriter(System.out));
                   }
                 To see objectweb.asm code that will generate bytecode for a given class:
                   java -cp "third_party/ow2_asm/lib/asm-5.0.1.jar:third_party/ow2_asm/lib/"\
                       "asm-util-5.0.1.jar:out/Debug/lib.java/jar_containing_yourclass.jar" \
                       org.objectweb.asm.util.ASMifier org.package.YourClassName
                */
                if (shouldUseThreadAnnotations) {
                    chain = new ThreadAssertionClassAdapter(chain);
                }
                if (shouldAssert) {
                    chain = new AssertionEnablerClassAdapter(chain);
                }
                if (shouldUseCustomResources) {
                    chain = new CustomResourcesClassAdapter(chain, reader.getClassName(),
                            reader.getSuperName(), classPathJarsClassLoader);
                }
                reader.accept(chain, 0);
                byte[] patchedByteCode = writer.toByteArray();
                writeZipEntry(tempStream, entry.getName(), patchedByteCode);
            }
        } catch (IOException e) {
            throw new RuntimeException(e);
        }
        try {
            Path src = Paths.get(tempJarPath);
            Path dest = Paths.get(outputJarPath);
            Files.move(src, dest, StandardCopyOption.REPLACE_EXISTING);
        } catch (IOException ioException) {
            throw new RuntimeException(ioException);
        }
    }

    private static byte[] readAllBytes(InputStream inputStream) throws IOException {
        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        int numRead = 0;
        byte[] data = new byte[BUFFER_SIZE];
        while ((numRead = inputStream.read(data, 0, data.length)) != -1) {
            buffer.write(data, 0, numRead);
        }
        return buffer.toByteArray();
    }

    /**
     * Loads a list of jars and returns a ClassLoader capable of loading all classes found in the
     * given jars.
     */
    private static ClassLoader loadJars(ArrayList<String> paths) {
        URL[] jarUrls = new URL[paths.size()];
        for (int i = 0; i < paths.size(); ++i) {
            try {
                jarUrls[i] = new File(paths.get(i)).toURI().toURL();
            } catch (MalformedURLException e) {
                throw new RuntimeException(e);
            }
        }
        return new URLClassLoader(jarUrls);
    }

    public static void main(String[] args) {
        // Invoke this script using //build/android/gyp/bytecode_processor.py
        String inputJarPath = args[0];
        String outputJarPath = args[1];
        boolean shouldAssert = args[2].equals("--enable-assert");
        boolean shouldUseCustomResources = args[3].equals("--enable-custom-resources");
        boolean shouldUseThreadAnnotations = args[4].equals("--enable-thread-annotations");

        // Load all jars that are on the classpath for the input jar for analyzing class hierarchy.
        ClassLoader classPathJarsClassLoader = null;
        if (shouldUseCustomResources) {
            ArrayList<String> classPathJarsPaths = new ArrayList<>();
            classPathJarsPaths.add(inputJarPath);
            classPathJarsPaths.addAll(Arrays.asList(Arrays.copyOfRange(args, 4, args.length)));
            classPathJarsClassLoader = loadJars(classPathJarsPaths);
        }
        process(inputJarPath, outputJarPath, shouldAssert, shouldUseCustomResources,
                shouldUseThreadAnnotations, classPathJarsClassLoader);
    }
}
