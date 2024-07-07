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
            f'nix eval --json nixpkgs#legacyPackages.x86_64-linux --apply "pkgs: with pkgs; [{pkg_names}]" --extra-experimental-features nix-command --extra-experimental-features flakes',
            shell=True,
        ).decode()
    )
    ld_library_path = []
    if old_val := os.getenv('LD_LIBRARY_PATH'):
        ld_library_path.append(old_val)
    ld_library_path.extend(x + '/lib' for x in pkgs)
    os.environ['LD_LIBRARY_PATH'] = ':'.join(ld_library_path)


def win_pipe_nowait(pipefd):
    if not iswin():
        raise ValueError('Tried to run win_pipe_nowait on a non-windows system')

    # https://stackoverflow.com/questions/34504970/non-blocking-read-on-os-pipe-on-windows
    import msvcrt
    from ctypes import windll, byref, wintypes, WinError
    from ctypes.wintypes import HANDLE, LPDWORD, BOOL

    PIPE_NOWAIT = wintypes.DWORD(0x00000001)

    SetNamedPipeHandleState = windll.kernel32.SetNamedPipeHandleState
    SetNamedPipeHandleState.argtypes = [HANDLE, LPDWORD, LPDWORD, LPDWORD]
    SetNamedPipeHandleState.restype = BOOL

    h = msvcrt.get_osfhandle(pipefd)

    res = windll.kernel32.SetNamedPipeHandleState(h, byref(PIPE_NOWAIT), None, None)
    if res == 0:
        raise WinError()
    return True


def win_get_long_path_name(path: str) -> str:
    from ctypes import create_unicode_buffer, windll, WinError

    buf = create_unicode_buffer(1024)
    res = windll.kernel32.GetLongPathNameW(path, buf, 1024)
    if res == 0:
        raise WinError()
    return buf.value
