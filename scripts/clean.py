#!/usr/bin/env python3
from pathlib import Path
import shutil

root = Path(__file__).resolve().parents[1]
for name in ("dist", "test-results", "playwright-report"):
    path = root / name
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
