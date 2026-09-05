#!/usr/bin/env python3
"""Check tools without changing the host configuration."""
from pathlib import Path
import platform
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
if "--help-targets" in sys.argv:
    for line in (root / "Makefile").read_text().splitlines():
        if ": ## " in line:
            name, description = line.split(": ## ", 1)
            print(f"{name:20} {description}")
    raise SystemExit(0)

failed = False
for command in (["rustc", "--version"], ["cargo", "--version"], ["node", "--version"], ["pnpm", "--version"], ["python3", "--version"], ["openssl", "version"]):
    if not shutil.which(command[0]):
        print(f"MISSING {command[0]}")
        failed = True
        continue
    result = subprocess.run(command, capture_output=True, text=True)
    print((result.stdout or result.stderr).strip().splitlines()[0])
    failed |= result.returncode != 0
if platform.system() == "Linux":
    for dependency in ("webkit2gtk-4.1", "gtk+-3.0", "libcurl", "openssl"):
        result = subprocess.run(["pkg-config", "--modversion", dependency], capture_output=True, text=True)
        print(f"{dependency}: {result.stdout.strip() if result.returncode == 0 else 'MISSING'}")
        failed |= result.returncode != 0
print(f"Platform: {platform.system()} {platform.machine()}")
raise SystemExit(int(failed))
