# Copyright 2017 The Chromium Authors. All rights reserved.
# Use of this source code is governed by a BSD-style license that can be
# found in the LICENSE file.
#
# gn_meta_sln.py
#   Helper utility to combine GN-generated Visual Studio projects into
#   a single meta-solution.

import os
import glob
import re
import sys
from shutil import copyfile

# Helpers
def EnsureExists(path):
    try:
        os.makedirs(path)
    except OSError:
        pass

def WriteLinesToFile(lines, file_name):
    EnsureExists(os.path.dirname(file_name))
    with open(file_name, "w") as f:
        f.writelines(lines)

def ExtractIdg(proj_file_name):
    result = []
    with open(proj_file_name) as proj_file:
        lines = iter(proj_file)
        for p_line in lines:
            if "<ItemDefinitionGroup" in p_line:
                while not "</ItemDefinitionGroup" in p_line:
                    result.append(p_line)
                    p_line = lines.next()
                result.append(p_line)
                return result

# [ (name, solution_name, vs_version), ... ]
configs = []

def GetVSVersion(solution_file):
    with open(solution_file) as f:
        f.readline()
        comment = f.readline().strip()
        return comment[-4:]

# Find all directories that can be used as configs (and record if they have VS
# files present)
for root, dirs, files in os.walk("out"):
    for out_dir in dirs:
        gn_file = os.path.join("out", out_dir, "build.ninja.d")
        if os.path.exists(gn_file):
            solutions = glob.glob(os.path.join("out", out_dir, "*.sln"))
            for solution in solutions:
                vs_version = GetVSVersion(solution)
                configs.append((out_dir, os.path.basename(solution),
                                vs_version))
    break

# Every project has a GUID that encodes the type. We only care about C++.
cpp_type_guid = "8BC9CEB8-8B4A-11D0-8D11-00A0C91BC942"

# Work around MSBuild limitations by always using a fixed arch.
hard_coded_arch = "x64"

# name -> [ (config, pathToProject, GUID, arch), ... ]
all_projects = {}
project_pattern = (r'Project\("\{' + cpp_type_guid +
                   r'\}"\) = "([^"]*)", "([^"]*)", "\{([^\}]*)\}"')

# We need something to work with. Typically, this will fail if no GN folders
# have IDE files
if len(configs) == 0:
    print("ERROR: At least one GN directory must have been built with --ide=vs")
    sys.exit()

# Filter out configs which don't match the name and vs version of the first.
name = configs[0][1]
vs_version = configs[0][2]

for config in configs:
    if config[1] != name or config[2] != vs_version:
        continue

    sln_lines = iter(open(os.path.join("out", config[0], config[1])))
    for sln_line in sln_lines:
        match_obj = re.match(project_pattern, sln_line)
        if match_obj:
            proj_name = match_obj.group(1)
            if not all_projects.has_key(proj_name):
                all_projects[proj_name] = []
            all_projects[proj_name].append((config[0], match_obj.group(2),
                                            match_obj.group(3)))

# We need something to work with. Typically, this will fail if no GN folders
# have IDE files
if len(all_projects) == 0:
    print("ERROR: At least one GN directory must have been built with --ide=vs")
    sys.exit()

# Create a new solution. We arbitrarily use the first config as the GUID source
# (but we need to match that behavior later, when we copy/generate the project
# files).
new_sln_lines = []
new_sln_lines.append(
    'Microsoft Visual Studio Solution File, Format Version 12.00\n')
new_sln_lines.append('# Visual Studio ' + vs_version + '\n')
for proj_name, proj_configs in all_projects.items():
    new_sln_lines.append('Project("{' + cpp_type_guid + '}") = "' + proj_name +
                         '", "' + proj_configs[0][1] + '", "{' +
                         proj_configs[0][2] + '}"\n')
    new_sln_lines.append('EndProject\n')

new_sln_lines.append('Global\n')
new_sln_lines.append(
    '\tGlobalSection(SolutionConfigurationPlatforms) = preSolution\n')
for config in configs:
    match = config[0] + '|' + hard_coded_arch
    new_sln_lines.append('\t\t' + match + ' = ' + match + '\n')
new_sln_lines.append('\tEndGlobalSection\n')
new_sln_lines.append(
    '\tGlobalSection(ProjectConfigurationPlatforms) = postSolution\n')
for proj_name, proj_configs in all_projects.items():
    proj_guid = proj_configs[0][2]
    for config in configs:
        match = config[0] + '|' + hard_coded_arch
        new_sln_lines.append('\t\t{' + proj_guid + '}.' + match +
                           '.ActiveCfg = ' + match + '\n')
        new_sln_lines.append('\t\t{' + proj_guid + '}.' + match +
                           '.Build.0 = ' + match + '\n')
new_sln_lines.append('\tEndGlobalSection\n')
new_sln_lines.append('\tGlobalSection(SolutionProperties) = preSolution\n')
new_sln_lines.append('\t\tHideSolutionNode = FALSE\n')
new_sln_lines.append('\tEndGlobalSection\n')
new_sln_lines.append('\tGlobalSection(NestedProjects) = preSolution\n')
new_sln_lines.append('\tEndGlobalSection\n')
new_sln_lines.append('EndGlobal\n')

# Write solution file
WriteLinesToFile(new_sln_lines, 'out/sln/' + name)

idg_hdr = "<ItemDefinitionGroup Condition=\"'$(Configuration)|$(Platform)'=='"

configuration_template = """    <ProjectConfiguration Include="{config}|{arch}">
      <Configuration>{config}</Configuration>
      <Platform>{arch}</Platform>
    </ProjectConfiguration>
"""

def FormatProjectConfig(config):
    return configuration_template.format(
        config = config[0], arch = hard_coded_arch)

# Now, bring over the project files
for proj_name, proj_configs in all_projects.items():
    # Paths to project and filter file in src and dst locations
    src_proj_path = os.path.join("out", proj_configs[0][0], proj_configs[0][1])
    dst_proj_path = os.path.join("out", "sln", proj_configs[0][1])
    src_filter_path = src_proj_path + ".filters"
    dst_filter_path = dst_proj_path + ".filters"

    # Copy the filter file unmodified
    EnsureExists(os.path.dirname(dst_proj_path))
    copyfile(src_filter_path, dst_filter_path)

    preferred_tool_arch = None
    config_arch = {}

    # Bring over the project file, modified with extra configs
    with open(src_proj_path) as src_proj_file:
        proj_lines = iter(src_proj_file)
        new_proj_lines = []
        for line in proj_lines:
            if "<ItemDefinitionGroup" in line:
                # This is a large group that contains many settings. We need to
                # replicate it, with conditions so it varies per configuration.
                idg_lines = []
                while not "</ItemDefinitionGroup" in line:
                    idg_lines.append(line)
                    line = proj_lines.next()
                idg_lines.append(line)
                for proj_config in proj_configs:
                    config_idg_lines = ExtractIdg(os.path.join("out",
                                                             proj_config[0],
                                                             proj_config[1]))
                    match = proj_config[0] + '|' + hard_coded_arch
                    new_proj_lines.append(idg_hdr + match + "'\">\n")
                    for idg_line in config_idg_lines[1:]:
                        new_proj_lines.append(idg_line)
            elif "ProjectConfigurations" in line:
                new_proj_lines.append(line)
                proj_lines.next()
                proj_lines.next()
                proj_lines.next()
                proj_lines.next()
                for config in configs:
                    new_proj_lines.append(FormatProjectConfig(config))

            elif "<OutDir" in line:
                new_proj_lines.append(line.replace(proj_configs[0][0],
                                                 "$(Configuration)"))
            elif "<PreferredToolArchitecture" in line:
                new_proj_lines.append("    <PreferredToolArchitecture>" +
                                      hard_coded_arch +
                                      "</PreferredToolArchitecture>\n")
            else:
                new_proj_lines.append(line)
        with open(dst_proj_path, "w") as new_proj:
            new_proj.writelines(new_proj_lines)

print('Wrote meta solution to out/sln/' + name)
