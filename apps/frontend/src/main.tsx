import "@mantine/core/styles.css";
import "./styles.css";
import { createTheme, MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createBackendClient } from "rust-backend-template-sdk";
import { BackendContext } from "./application/backend";
import { router } from "./bootstrap/router";
import {
	Authentication,
	identityQueryKey,
} from "./infrastructure/authentication";
import { csrfToken } from "./infrastructure/browser";

const root = document.getElementById("root");
if (!root) throw new Error("Missing application root");

const queries = new QueryClient({
	defaultOptions: { queries: { retry: false } },
});
const backend = createBackendClient(window.location.origin, csrfToken);
backend.use({
	onResponse({ response, schemaPath }) {
		if (response.status === 401 && schemaPath !== "/v1/me") {
			void queries.invalidateQueries({
				queryKey: identityQueryKey,
				exact: true,
			});
		}
	},
});
const services = {
	client: backend,
	authentication: new Authentication(backend),
};

const theme = createTheme({ primaryColor: "teal", defaultRadius: "md" });
createRoot(root).render(
	<StrictMode>
		<MantineProvider theme={theme}>
			<QueryClientProvider client={queries}>
				<BackendContext.Provider value={services}>
					<RouterProvider router={router} />
				</BackendContext.Provider>
			</QueryClientProvider>
		</MantineProvider>
	</StrictMode>,
);
