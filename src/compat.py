import json
import os
import subprocess

import sys


def iswin():
    return sys.platform == 'win32'


def islinux():
    return sys.platform == 'linux'


def ismac():
    return sys.platform == 'darwin'


def is_frozen():
    return getattr(sys, 'frozen', False)


def chmod_x(path):
    if islinux() or ismac():
        path = str(path)
        os.chmod(path, os.stat(path).st_mode | 0o111)


def perform_forbidden_nixery():
    if not islinux() or not os.path.isfile('/etc/NIXOS'):
        return

    print('Performing forbidden nixery')
    pkg_names = [
        'xorg.libX11',
        'xorg.libXext',
        'xorg.libXcursor',
        'xorg.libXrandr',
        'xorg.libXxf86vm',
        'libpulseaudio',
        'libGL',
        'glfw',
        'openal',
    ]
    pkg_names = ' '.join(pkg_names)
    pkgs = json.loads(
        subprocess.check_output(
            f'nix eval --json nixpkgs#legacyPackages.x86_64-linux --apply "pkgs: with pkgs; [{pkg_names}]"',
            shell=True,
        ).decode()
    )
    ld_library_path = []
    if old_val := os.getenv('LD_LIBRARY_PATH'):
        ld_library_path.append(old_val)
    ld_library_path.extend(x + '/lib' for x in pkgs)
    os.environ['LD_LIBRARY_PATH'] = ':'.join(ld_library_path)


__all__ = [
    'iswin',
    'islinux',
    'ismac',
    'is_frozen',
    'chmod_x',
    'perform_forbidden_nixery',
]
