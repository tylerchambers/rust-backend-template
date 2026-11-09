"""Guard standalone Cargo recipes without requiring service tooling."""

import os
import sys

from preflight import check_rust

check_rust()
os.execvp("cargo", ["cargo", *sys.argv[1:]])
