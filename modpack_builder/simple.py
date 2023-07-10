import json
import os
import sys
from hashlib import sha1
from pathlib import Path

classpath = [
    'libraries/cpw/mods/securejarhandler/2.1.4/securejarhandler-2.1.4.jar',
    'libraries/ca/weblite/java-objc-bridge/1.1/java-objc-bridge-1.1.jar',
    'libraries/org/ow2/asm/asm/9.3/asm-9.3.jar',
    'libraries/org/ow2/asm/asm-commons/9.3/asm-commons-9.3.jar',
    'libraries/org/ow2/asm/asm-tree/9.3/asm-tree-9.3.jar',
    'libraries/org/ow2/asm/asm-util/9.3/asm-util-9.3.jar',
    'libraries/org/ow2/asm/asm-analysis/9.3/asm-analysis-9.3.jar',
    'libraries/net/minecraftforge/accesstransformers/8.0.4/accesstransformers-8.0.4.jar',
    'libraries/org/antlr/antlr4-runtime/4.9.1/antlr4-runtime-4.9.1.jar',
    'libraries/net/minecraftforge/eventbus/6.0.3/eventbus-6.0.3.jar',
    'libraries/net/minecraftforge/forgespi/6.0.0/forgespi-6.0.0.jar',
    'libraries/net/minecraftforge/coremods/5.0.1/coremods-5.0.1.jar',
    'libraries/cpw/mods/modlauncher/10.0.8/modlauncher-10.0.8.jar',
    'libraries/net/minecraftforge/unsafe/0.2.0/unsafe-0.2.0.jar',
    'libraries/com/electronwill/night-config/core/3.6.4/core-3.6.4.jar',
    'libraries/com/electronwill/night-config/toml/3.6.4/toml-3.6.4.jar',
    'libraries/org/apache/maven/maven-artifact/3.8.5/maven-artifact-3.8.5.jar',
    'libraries/net/jodah/typetools/0.8.3/typetools-0.8.3.jar',
    'libraries/net/minecrell/terminalconsoleappender/1.2.0/terminalconsoleappender-1.2.0.jar',
    'libraries/org/jline/jline-reader/3.12.1/jline-reader-3.12.1.jar',
    'libraries/org/jline/jline-terminal/3.12.1/jline-terminal-3.12.1.jar',
    'libraries/org/spongepowered/mixin/0.8.5/mixin-0.8.5.jar',
    'libraries/org/openjdk/nashorn/nashorn-core/15.3/nashorn-core-15.3.jar',
    'libraries/net/minecraftforge/JarJarSelector/0.3.16/JarJarSelector-0.3.16.jar',
    'libraries/net/minecraftforge/JarJarMetadata/0.3.16/JarJarMetadata-0.3.16.jar',
    'libraries/cpw/mods/bootstraplauncher/1.1.2/bootstraplauncher-1.1.2.jar',
    'libraries/net/minecraftforge/JarJarFileSystems/0.3.16/JarJarFileSystems-0.3.16.jar',
    'libraries/net/minecraftforge/fmlloader/1.19.2-43.2.14/fmlloader-1.19.2-43.2.14.jar',
    'libraries/com/mojang/logging/1.0.0/logging-1.0.0.jar',
    'libraries/com/mojang/blocklist/1.0.10/blocklist-1.0.10.jar',
    'libraries/ru/tln4/empty/0.1/empty-0.1.jar',
    'libraries/com/github/oshi/oshi-core/5.8.5/oshi-core-5.8.5.jar',
    'libraries/net/java/dev/jna/jna/5.10.0/jna-5.10.0.jar',
    'libraries/net/java/dev/jna/jna-platform/5.10.0/jna-platform-5.10.0.jar',
    'libraries/org/slf4j/slf4j-api/1.8.0-beta4/slf4j-api-1.8.0-beta4.jar',
    'libraries/org/apache/logging/log4j/log4j-slf4j18-impl/2.17.0/log4j-slf4j18-impl-2.17.0.jar',
    'libraries/com/ibm/icu/icu4j/70.1/icu4j-70.1.jar',
    'libraries/com/mojang/javabridge/1.2.24/javabridge-1.2.24.jar',
    'libraries/net/sf/jopt-simple/jopt-simple/5.0.4/jopt-simple-5.0.4.jar',
    'libraries/io/netty/netty-common/4.1.77.Final/netty-common-4.1.77.Final.jar',
    'libraries/io/netty/netty-buffer/4.1.77.Final/netty-buffer-4.1.77.Final.jar',
    'libraries/io/netty/netty-codec/4.1.77.Final/netty-codec-4.1.77.Final.jar',
    'libraries/io/netty/netty-handler/4.1.77.Final/netty-handler-4.1.77.Final.jar',
    'libraries/io/netty/netty-resolver/4.1.77.Final/netty-resolver-4.1.77.Final.jar',
    'libraries/io/netty/netty-transport/4.1.77.Final/netty-transport-4.1.77.Final.jar',
    'libraries/io/netty/netty-transport-native-unix-common/4.1.77.Final/netty-transport-native-unix-common-4.1.77.Final.jar',
    'libraries/io/netty/netty-transport-classes-epoll/4.1.77.Final/netty-transport-classes-epoll-4.1.77.Final.jar',
    'libraries/io/netty/netty-transport-native-epoll/4.1.77.Final/netty-transport-native-epoll-4.1.77.Final-linux-x86_64.jar',
    'libraries/io/netty/netty-transport-native-epoll/4.1.77.Final/netty-transport-native-epoll-4.1.77.Final-linux-aarch_64.jar',
    'libraries/com/google/guava/failureaccess/1.0.1/failureaccess-1.0.1.jar',
    'libraries/com/google/guava/guava/31.0.1-jre/guava-31.0.1-jre.jar',
    'libraries/org/apache/commons/commons-lang3/3.12.0/commons-lang3-3.12.0.jar',
    'libraries/commons-io/commons-io/2.11.0/commons-io-2.11.0.jar',
    'libraries/commons-codec/commons-codec/1.15/commons-codec-1.15.jar',
    'libraries/com/mojang/brigadier/1.0.18/brigadier-1.0.18.jar',
    'libraries/com/mojang/datafixerupper/5.0.28/datafixerupper-5.0.28.jar',
    'libraries/com/google/code/gson/gson/2.8.9/gson-2.8.9.jar',
    'libraries/by/ely/authlib/3.11.49.2/authlib-3.11.49.2.jar',
    'libraries/org/apache/commons/commons-compress/1.21/commons-compress-1.21.jar',
    'libraries/org/apache/httpcomponents/httpclient/4.5.13/httpclient-4.5.13.jar',
    'libraries/commons-logging/commons-logging/1.2/commons-logging-1.2.jar',
    'libraries/org/apache/httpcomponents/httpcore/4.4.14/httpcore-4.4.14.jar',
    'libraries/it/unimi/dsi/fastutil/8.5.6/fastutil-8.5.6.jar',
    'libraries/org/apache/logging/log4j/log4j-api/2.17.0/log4j-api-2.17.0.jar',
    'libraries/org/apache/logging/log4j/log4j-core/2.17.0/log4j-core-2.17.0.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos-arm64.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos-arm64.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos-arm64.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos-arm64.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos-arm64.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos-arm64.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-linux.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-linux-arm64.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-windows.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-windows-x86.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-windows-arm64.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos.jar',
    'libraries/org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos-arm64.jar',
    'libraries/com/mojang/text2speech/1.16.7/text2speech-1.16.7.jar',
]
cfg_copy_extra = [
    'authlib-injector.jar',
    'servers.dat',
    'config/radium.properties',
    'config/quark-common.toml',
    'config/emi.css',
    'config/carryon-common.toml',
    'config/defaultoptions',
]
target_dir = Path(sys.argv[1])
version_data_path = os.path.expanduser('~/1.19.2-forge-43.2.14.json')

with open(version_data_path) as f:
    version_data = json.load(f)


def hash_dir(directory: Path, exclude: list[Path] = None) -> dict[Path, str]:
    if exclude is None:
        exclude = []
    res = {}
    for path in directory.rglob('*'):
        if path.is_dir():
            continue
        relpath = path.relative_to(directory)
        if relpath in exclude:
            continue
        with open(path, 'rb') as f:
            res[relpath] = sha1(f.read()).hexdigest()
    return res


def create_index() -> None:
    print('Creating index file...')
    hashes = {
        str(k): v for k, v in hash_dir(target_dir, exclude=[Path('index.json')]).items()
    }
    index = {
        'version': version_data['jar'],
        'asset_index': version_data['assetIndex']['id'],
        'main_class': version_data['mainClass'],
        'classpath': classpath,
        'java_args': version_data['arguments']['jvm'],
        'game_args': version_data['arguments']['game'],
        'include': [
            'libraries',
            'mods',
            'client.jar',
            *cfg_copy_extra,
        ],
        'objects': hashes,
    }
    with open(target_dir / 'index.json', 'w') as f:
        json.dump(index, f)


if __name__ == '__main__':
    create_index()
    os.chdir(target_dir)
    os.system('zip -r ../modpackzip/modpack.zip config mods')
