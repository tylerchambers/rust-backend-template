"""Exercise real linters on isolated bad fixtures, never mutate application sources."""

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from preflight import check_rust  # noqa: E402

check_rust()


def execute(command, cwd, *, code=0, contains=(), source=None):
    result = subprocess.run(
        command, cwd=cwd, input=source, text=True, capture_output=True, timeout=120
    )
    output = result.stdout + result.stderr
    assert (result.returncode == 0) == (code == 0), output
    for marker in contains:
        assert marker in output, (marker, output)
    print(f"PASS exit={result.returncode}: {' '.join(map(str, command))}")
    if code:
        print(output)
    return output


def main():
    (ROOT / ".local").mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="lint-controls-", dir=ROOT / ".local"
    ) as temp:
        root = Path(temp)
        policy = (ROOT / "Cargo.toml").read_text().split("[workspace.lints.rust]", 1)[1]
        policy = (
            "[workspace.lints.rust]" + policy.split("[workspace.dependencies]", 1)[0]
        )
        (root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["probe"]\nresolver = "3"\n' + policy
        )
        crate = root / "probe"
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text(
            '[package]\nname = "lint-probe"\nversion = "0.0.0"\nedition = "2024"\n'
            "[lints]\nworkspace = true\n"
        )
        source = crate / "src/lib.rs"
        source.write_text(
            "pub fn unchecked(value: Option<u8>) { value.unwrap(); }\n"
            'pub fn expected(value: Option<u8>) { value.expect("bad"); }\n'
            "pub fn debug_output() { dbg!(1); }\n"
            "pub fn unfinished() { todo!(); }\n"
            'pub fn output() { println!("stray"); }\n'
            "pub fn undocumented() -> Result<(), std::io::Error> { Ok(()) }\n"
        )
        clippy = [
            "cargo",
            "clippy",
            "--offline",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]
        execute(
            clippy,
            root,
            code=1,
            contains=[
                "clippy::unwrap_used",
                "clippy::expect_used",
                "clippy::dbg_macro",
                "clippy::todo",
                "clippy::print_stdout",
                "clippy::missing_errors_doc",
            ],
        )
        source.write_text("#[allow(unsafe_code)]\npub unsafe fn forbidden() {}\n")
        execute(clippy, root, code=1, contains=["forbid", "unsafe_code"])
        source.write_text(
            "#[cfg(test)]\nmod tests {\n"
            '    #[allow(clippy::unwrap_used, clippy::expect_used, reason = "Assertion fixture")]\n'
            "    #[test]\n    fn assertions() {\n"
            '        let value = "1".parse::<u8>();\n'
            "        assert_eq!(value.unwrap(), 1);\n"
            '        assert_eq!("2".parse::<u8>().expect("fixture"), 2);\n'
            "    }\n}\n"
        )
        execute(clippy, root)
        execute(["cargo", "test", "--offline"], root)

        ruff = str(ROOT / ".venv/bin/ruff")
        execute(
            [ruff, "check", "--stdin-filename", "scripts/probe.py", "-"],
            ROOT,
            code=1,
            contains=["F821"],
            source="print(undefined_name)\n",
        )
        execute(
            [ruff, "format", "--check", "--stdin-filename", "tests/probe.py", "-"],
            ROOT,
            code=1,
            contains=[],
            source="values=[1,2]\n",
        )
        (root / "biome.json").write_bytes((ROOT / "biome.json").read_bytes())
        sdk = root / "sdk"
        (sdk / "src").mkdir(parents=True)
        biome = [
            "bun",
            str(ROOT / "sdk/node_modules/@biomejs/biome/bin/biome"),
            "check",
            "--config-path",
            str(root),
            "--error-on-warnings",
        ]
        probe = sdk / "probe.ts"
        probe.write_text("export const value: any = 1;\n")
        execute([*biome, str(probe)], root, code=1, contains=["noExplicitAny"])
        probe.unlink()
        javascript = sdk / "probe.mjs"
        javascript.write_text("export const value={a:1};\n")
        execute([*biome, str(javascript)], root, code=1, contains=["Formatter"])
        generated = sdk / "src/schema.d.ts"
        before = (ROOT / "sdk/src/schema.d.ts").read_bytes()
        generated.write_bytes(before)
        execute([*biome, "--write", "sdk"], root)
        assert generated.read_bytes() == before, (
            "Biome touched generator-owned declarations"
        )

        deny = str(ROOT / ".local/tools/bin/cargo-deny")
        config = (ROOT / "deny.toml").read_text()
        for name, mutation, marker in [
            ("bans", config.replace("[bans]", '[bans]\ndeny = ["serde"]'), "banned"),
            (
                "sources",
                config.replace(
                    'allow-registry = ["https://github.com/rust-lang/crates.io-index"]',
                    "allow-registry = []",
                ),
                "source-not-allowed",
            ),
            ("licenses", config.replace('"MIT", ', ""), "rejected"),
        ]:
            target = root / f"{name}.toml"
            target.write_text(mutation)
            execute(
                [
                    deny,
                    "--locked",
                    "--config",
                    str(target),
                    "check",
                    name,
                    "--deny",
                    "warnings",
                    "--hide-inclusion-graph",
                ],
                ROOT,
                code=1,
                contains=[marker],
            )

    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "metadata", "--no-deps", "--locked", "--format-version", "1"],
            cwd=ROOT,
            text=True,
        )
    )
    print(
        f"PASS: policy controls; {len(metadata['workspace_members'])} workspace crates checked by lint.py"
    )


if __name__ == "__main__":
    os.environ["CARGO_TERM_COLOR"] = "never"
    main()
