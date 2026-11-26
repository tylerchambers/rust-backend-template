import { expect, test } from "bun:test";

const layers = {
	domain: [],
	application: ["domain"],
	api: ["domain", "application"],
	infrastructure: ["domain", "application"],
	bootstrap: ["domain", "application", "api", "infrastructure"],
	server: ["bootstrap"],
	worker: ["bootstrap"],
	cli: ["bootstrap"],
};

const frameworkDeps = ["axum", "tower", "tokio", "reqwest", "lettre"];

test("workspace crates inherit lint policy and depend only on permitted layers", async () => {
	const { packages } =
		await Bun.$`cargo metadata --no-deps --locked --format-version 1`.json();
	for (const pkg of packages) {
		expect(Object.hasOwn(layers, pkg.name), pkg.name).toBe(true);
		const manifest = Bun.TOML.parse(await Bun.file(pkg.manifest_path).text());
		expect(manifest.lints, pkg.name).toEqual({ workspace: true });
		for (const { name, kind } of pkg.dependencies) {
			if (kind === "dev") continue;
			const edge = `${pkg.name} -> ${name}`;
			if (Object.hasOwn(layers, name))
				expect(layers[pkg.name], edge).toContain(name);
			if (["domain", "application"].includes(pkg.name)) {
				expect(frameworkDeps, edge).not.toContain(name);
			}
			if (["domain", "application", "api"].includes(pkg.name)) {
				expect(["sqlx", "riverqueue", "redis"], edge).not.toContain(name);
			}
		}
	}
});
