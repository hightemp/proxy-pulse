#!/usr/bin/env python3
"""Keep loopback test traffic independent of shell proxy settings."""
import os
import subprocess
import shutil

environment = {key: value for key, value in os.environ.items() if key.lower() not in ("http_proxy", "https_proxy", "all_proxy")}
if not environment.get("CI") and not environment.get("CHROME_PATH"):
    installed = shutil.which("google-chrome")
    if installed:
        environment["CHROME_PATH"] = installed
raise SystemExit(subprocess.call([shutil.which("pnpm") or "pnpm", "test:ui"], env=environment))
