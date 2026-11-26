import { dirname, join } from "node:path";

const result = await Bun.build({
	entrypoints: ["generate-entry.ts"],
	outdir: dirname(Bun.argv[3]),
	target: "bun",
	plugins: [
		{
			name: "typescript-compiler-api",
			setup(builder) {
				builder.onResolve({ filter: /^typescript$/ }, () => ({
					path: Bun.resolveSync("typescript-compat", import.meta.dir),
				}));
			},
		},
	],
});
if (!result.success) throw new Error("OpenAPI generator bundle failed");
// The compiler-API alias exists only in this freshly built bundle.
await import(join(dirname(Bun.argv[3]), "generate-entry.js"));
