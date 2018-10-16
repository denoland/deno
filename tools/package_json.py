import json
import os

root_path = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))


def get_typescript_version():
    with open(os.path.join(root_path, 'package.json')) as f:
        package_json_data = json.load(f)
        return str(package_json_data["devDependencies"]["typescript"])
