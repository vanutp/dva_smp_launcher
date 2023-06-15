import os

with open('build_cfg.py', 'w') as f:
    for env in ["CLIENT_ID", "CLIENT_SECRET", "SERVER_BASE"]:
        f.write(f'{env} = "{os.getenv(env)}"{os.linesep}')
