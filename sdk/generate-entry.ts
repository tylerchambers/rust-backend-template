import openapiTS, { astToString } from "openapi-typescript";

const schema = await openapiTS(Bun.pathToFileURL(Bun.argv[2]));
await Bun.write(Bun.argv[3], astToString(schema));
