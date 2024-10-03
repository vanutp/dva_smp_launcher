#!/usr/bin/env python3

# this script is run over the files in https://github.com/PrismLauncher/meta-launcher/tree/master
# to generate the library-overrides.json file
import json
import os


major_version_groups = ['org.lwjgl', 'org.lwjgl.lwjgl']

# the launcher will remove all libraries with these groups
# and put all libraries in the matching json file in their place
groups_to_remove = ['org.lwjgl', 'org.lwjgl.lwjgl', 'net.java.jinput', 'net.java.jutils']
artifact_id_to_match = 'lwjgl'
match_ignore_os = ['osx']
directory_name = {
    'org.lwjgl.lwjgl': 'org.lwjgl',
    'org.lwjgl': 'org.lwjgl3',
}

combined_json = {}

combined_json['groups_to_remove'] = groups_to_remove
combined_json['artifact_id_to_match'] = artifact_id_to_match
combined_json['match_ignore_os'] = match_ignore_os
combined_json['overrides'] = []
for major_version_group in major_version_groups:
    directory = directory_name[major_version_group]
    for filename in os.listdir(directory):
        if filename.endswith('.json') and filename not in ['index.json', 'package.json']:
            filepath = os.path.join(directory, filename)
            with open(filepath, 'r') as file:
                json_content = json.load(file)
                combined_json['overrides'].append(json_content)

with open('library-patches.json', 'w') as outfile:
    json.dump(combined_json, outfile, indent=4)

print("Combined JSON file created successfully.")
