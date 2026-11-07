import { resolve } from "node:path";
const result = await Bun.build({
  entrypoints: ["generate-entry.ts"],
  outdir: "../.local/generator",
  target: "bun",
  define: { "import.meta.env.SOURCE_URL": JSON.stringify(new URL("./generate-entry.ts", import.meta.url).href) },
  plugins: [{ name: "typescript-compiler-api", setup(builder) {
    builder.onResolve({ filter: /^typescript$/ }, () => ({ path: Bun.resolveSync("typescript-compat", import.meta.dir) }));
  } }],
});
if (!result.success) throw new Error("OpenAPI generator bundle failed");
await import(resolve("../.local/generator/generate-entry.js"));
