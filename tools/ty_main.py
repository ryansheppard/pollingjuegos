import os
import sys
from pathlib import Path

import ty

binary = Path(ty.__file__).resolve().parents[4] / "bin" / "ty"

if wd := os.environ.get("BUILD_WORKSPACE_DIRECTORY"):
    os.chdir(wd)

os.execv(binary, [binary, *sys.argv[1:]])
