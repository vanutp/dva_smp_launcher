import os

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
    if islinux():
        path = str(path)
        os.chmod(path, os.stat(path).st_mode | 0o111)


__all__ = ['iswin', 'islinux', 'ismac', 'is_frozen', 'chmod_x']
