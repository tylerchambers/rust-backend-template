import openapiTS, { astToString } from "openapi-typescript";
const schema = await openapiTS(new URL("../openapi.json", import.meta.env.SOURCE_URL));
await Bun.write("src/schema.d.ts", astToString(schema));
