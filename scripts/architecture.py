"""Executable dependency-direction contract; no source-text heuristics."""

import copy
import json
import subprocess

LAYERS = {
    "domain": set(),
    "application": {"domain"},
    "api": {"domain", "application"},
    "infrastructure": {"domain", "application"},
    "bootstrap": {"domain", "application", "infrastructure", "api"},
    "server": {"bootstrap"},
    "worker": {"bootstrap"},
    "cli": {"bootstrap"},
}


def violations(packages):
    failures = []
    for package in packages:
        name = package["name"]
        if name not in LAYERS:
            continue
        for dependency in package["dependencies"]:
            if dependency["kind"] == "dev":
                continue
            target = dependency["name"]
            if target in LAYERS and target not in LAYERS[name]:
                failures.append(f"{name} -> {target}")
            if name in {"domain", "application"} and target in {
                "axum",
                "tower",
                "tokio",
                "reqwest",
                "lettre",
            }:
                failures.append(f"{name} -> {target}")
            if name in {"domain", "application", "api"} and target in {
                "sqlx",
                "riverqueue",
                "redis",
            }:
                failures.append(f"{name} -> {target}")
    return failures


metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], text=True
    )
)
assert not violations(metadata["packages"]), violations(metadata["packages"])
mutation = copy.deepcopy(metadata["packages"])
next(p for p in mutation if p["name"] == "domain")["dependencies"].append(
    {"kind": None, "name": "infrastructure"}
)
assert violations(mutation), "Boundary violation mutation was not rejected"
http_mutation = copy.deepcopy(metadata["packages"])
next(p for p in http_mutation if p["name"] == "domain")["dependencies"].append(
    {"kind": None, "name": "axum"}
)
assert violations(http_mutation), "Domain HTTP framework mutation was not rejected"
print("PASS: dependency direction and intentional-violation control")
