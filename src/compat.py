import sys


def iswin():
    return sys.platform == 'win32'


def islinux():
    return sys.platform == 'linux'


def ismac():
    return sys.platform == 'darwin'


__all__ = ['iswin', 'islinux', 'ismac']
