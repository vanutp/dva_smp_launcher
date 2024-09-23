from pathlib import Path
import click
from generate import generate
import asyncio


@click.command()
@click.argument("spec_file", type=click.Path(exists=True))
@click.option("--output-dir", type=click.Path(), default="./modpacks")
@click.option("--work-dir", type=click.Path(), default="./workdir")
def main(spec_file: str, output_dir: str, work_dir: str):
    spec_file_path = Path(spec_file)
    output_dir_path = Path(output_dir)
    work_dir_path = Path(work_dir)
    asyncio.run(generate(spec_file_path, output_dir_path, work_dir_path))


if __name__ == "__main__":
    main()
