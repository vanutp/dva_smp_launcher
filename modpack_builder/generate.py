from pydantic import BaseModel
from pathlib import Path
from portablemc.standard import Version, Context
from portablemc.forge import ForgeVersion
from portablemc.fabric import FABRIC_API, FabricVersion

import httpx
import logging
import json
import shutil

from utils import hash_files, download_authlib_injector


log_format = "%(name)s - %(levelname)s - %(message)s"
logging.basicConfig(level=logging.INFO, format=log_format)
logger = logging.getLogger(__name__)


class Object(BaseModel):
    path: str
    sha1: str
    url: str


class ModpackIndex(BaseModel):
    modpack_name: str
    modpack_version: str
    include: list[str]
    include_no_overwrite: list[str]
    objects: list[Object]
    resources_url_base: str | None
    java_version: str


class ModpacksIndex(BaseModel):
    modpack_indexes: list[ModpackIndex]


class ModpackSpec(BaseModel):
    modpack_name: str
    modpack_version: str | None = None
    minecraft_version: str
    loader_name: str | None = None
    loader_version: str | None = None
    include: list[str]
    include_no_overwrite: list[str]
    include_from: str | None = None
    replace_download_urls: bool = True
    exec_before: str | None = None
    exec_after: str | None = None
    java_version: str | None = None

    def get_modpack_version(
        self, modpacks_dir: Path, fetched_modpack_version: str | None
    ) -> str:
        modpack_version = self.modpack_version or fetched_modpack_version
        if not modpack_version:
            logger.warning(
                f"No modpack version provided for '{self.modpack_name}', defaulting to '1'"
            )
            modpack_version = "1"
        else:
            modpack_version = str(int(modpack_version) + 1)

        return modpack_version

    def get_portablemc_version(
        self, context: Context
    ) -> Version | ForgeVersion | FabricVersion | None:
        if not self.loader_name or self.loader_name == "vanilla":
            return Version(self.minecraft_version, context=context)
        elif self.loader_name == "forge":
            return ForgeVersion(
                self.loader_version or self.minecraft_version, context=context
            )
        elif self.loader_name == "fabric":
            return FabricVersion(
                FABRIC_API,
                self.minecraft_version,
                self.loader_version,
                "fabric",
                context=context,
            )
        else:
            logger.error(f"Unsupported loader '{self.loader_name}'")
            return None

    def replace_download_urls_version(
        self, modpacks_dir: Path, version_id: str, download_server_base: str
    ):
        version_metadata_path = (
            modpacks_dir
            / self.modpack_name
            / "versions"
            / version_id
            / f"{version_id}.json"
        )
        version_metadata = json.loads(version_metadata_path.read_text())

        for library in version_metadata["libraries"]:
            try:
                url = library["downloads"]["artifact"]["url"]
                if isinstance(url, str) and url:
                    *url_base, url_without_base = url.split("/", 3)
                    url_base = "/".join(url_base)

                    if not download_server_base.startswith(url_base):
                        library["downloads"]["artifact"][
                            "url"
                        ] = f"{download_server_base}/{self.modpack_name}/libraries/{url_without_base}"
            except KeyError:
                pass

        try:
            filename = version_metadata["assetIndex"]["id"] + ".json"
            version_metadata["assetIndex"][
                "url"
            ] = f"{download_server_base}/{self.modpack_name}/assets/indexes/{filename}"
        except KeyError:
            pass

        with open(version_metadata_path, "w") as f:
            json.dump(version_metadata, f)

    def replace_download_urls_versions(
        self, modpacks_dir: Path, download_server_base: str
    ):
        versions_path = modpacks_dir / self.modpack_name / "versions"
        for version_id in versions_path.iterdir():
            if not version_id.is_dir():
                continue
            self.replace_download_urls_version(
                modpacks_dir, version_id.name, download_server_base
            )

    async def generate_objects(self, modpack_dir: Path) -> dict[str, str]:
        files: list[Path] = []
        missing_includes: list[str] = []
        for include in self.include + self.include_no_overwrite:
            include_path_absolute = modpack_dir / include

            if self.include_from:
                include_from_entry = Path(self.include_from) / include
                if include_from_entry.is_dir():
                    shutil.rmtree(include_path_absolute, ignore_errors=True)
                    include_path_absolute.mkdir(parents=True, exist_ok=True)
                    shutil.copytree(
                        include_from_entry, include_path_absolute, dirs_exist_ok=True
                    )
                elif include_from_entry.is_file():
                    include_path_absolute.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(include_from_entry, include_path_absolute)
                else:
                    missing_includes.append(include)
                    logger.error(
                        f"Include '{include}' not found in '{self.include_from}'"
                    )
                    continue

            if include_path_absolute.is_dir():
                logger.info(f"Adding directory '{include}'")
                for file_path in include_path_absolute.rglob("*"):
                    if file_path.is_file():
                        files.append(file_path)
            elif include_path_absolute.is_file():
                logger.info(f"Adding file '{include}'")
                files.append(include_path_absolute)
            else:
                missing_includes.append(include)
                logger.error(f"Include '{include}' not found")

        logger.info("Hashing objects")
        objects = await hash_files(files)
        objects = {
            str(file_path.relative_to(modpack_dir).as_posix()): hash_value
            for file_path, hash_value in objects.items()
        }
        return objects

    async def generate(
        self,
        modpacks_dir: Path,
        work_dir: Path,
        authlib_injector_path: Path,
        fetched_modpack_version: str | None = None,
        download_server_base: str | None = None,
    ) -> ModpackIndex:
        modpack_version = self.get_modpack_version(
            modpacks_dir, fetched_modpack_version
        )

        modpack_dir = modpacks_dir / self.modpack_name
        assets_dir = modpacks_dir / "assets"
        jvm_dir = work_dir / self.modpack_name / "jvm"

        context = Context(modpack_dir, work_dir)
        context.assets_dir = assets_dir
        context.jvm_dir = jvm_dir

        loader_name = self.loader_name or "vanilla"

        version = self.get_portablemc_version(context)

        logger.info(
            f"Generating '{loader_name}' modpack '{self.modpack_name}' with version '{modpack_version}'"
        )
        _ = version.install()

        if self.replace_download_urls:
            if not download_server_base:
                logger.error(
                    "Download server base URL not provided, skipping download URLs replacement"
                )
            else:
                logger.info(f"Replacing download URLs with '{download_server_base}'")
                self.replace_download_urls_versions(modpacks_dir, download_server_base)

        shutil.copy2(authlib_injector_path, modpack_dir / "authlib-injector.jar")

        objects_hashes = await self.generate_objects(modpack_dir)
        objects = [
            Object(
                path=path,
                sha1=hash_value,
                url=f"{download_server_base}/{self.modpack_name}/{path}",
            )
            for path, hash_value in objects_hashes.items()
        ]

        java_version = (
            self.java_version
            if self.java_version
            else str(version._metadata["javaVersion"]["majorVersion"])
        )

        return ModpackIndex(
            modpack_name=self.modpack_name,
            modpack_version=modpack_version,
            include=self.include,
            include_no_overwrite=self.include_no_overwrite,
            objects=objects,
            resources_url_base=(
                f"{download_server_base}/assets/objects"
                if self.replace_download_urls
                else None
            ),
            java_version=java_version,
        )


class Spec(BaseModel):
    modpack_version_fetch_url: str | None = None
    download_server_base: str | None = None
    modpacks: list[ModpackSpec]
    exec_before_all: str | None = None
    exec_after_all: str | None = None

    async def fetch_modpacks_versions(self) -> dict[str, str]:
        if not self.modpack_version_fetch_url:
            logger.info("No modpack version fetch URL provided")
            return {}
        async with httpx.AsyncClient() as client:
            logger.info(
                f"Fetching modpack versions from '{self.modpack_version_fetch_url}'"
            )
            resp = await client.get(self.modpack_version_fetch_url)
            if resp.status_code == 404:
                logger.warning("Existing versions not found")
                return {}
            resp.raise_for_status()
            versions = {
                x["modpack_name"]: str(int(x["modpack_version"]) + 1)
                for x in resp.json()
                if "modpack_version" in x
            }
            return versions

    async def generate(self, modpacks_dir: Path, work_dir: Path) -> ModpacksIndex:
        modpacks_versions = await self.fetch_modpacks_versions()

        authlib_injector_path = await download_authlib_injector(work_dir)

        indexes: list[ModpackIndex] = []
        for modpack_spec in self.modpacks:
            indexes.append(
                await modpack_spec.generate(
                    modpacks_dir,
                    work_dir,
                    authlib_injector_path,
                    modpacks_versions.get(modpack_spec.modpack_name),
                    self.download_server_base,
                )
            )
        logger.info("Modpacks generation complete")

        return ModpacksIndex(modpack_indexes=indexes)


async def generate(spec_file_path: Path, modpacks_dir: Path, work_dir: Path):
    logger.info(f"Reading spec file from '{spec_file_path}'")
    spec = Spec.model_validate_json(spec_file_path.read_text())

    index = await spec.generate(modpacks_dir, work_dir)
    with open(modpacks_dir / "index.json", "w") as f:
        f.write(index.model_dump_json())
