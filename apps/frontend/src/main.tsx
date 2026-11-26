import "@mantine/core/styles.css";
import "../desk.css";
import "./styles.css";
import { createTheme, MantineProvider } from "@mantine/core";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createBackendClient } from "rust-backend-template-sdk";
import { BackendContext } from "./application/backend";
import { createWorkspaceRouter } from "./bootstrap/router";
import {
	Authentication,
	identityQueryKey,
} from "./infrastructure/authentication";
import { csrfToken } from "./infrastructure/browser";
import { ConfirmSignIn } from "./presentation/ConfirmSignIn";
import { ProjectDesk } from "./presentation/ProjectDesk";

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

const theme = createTheme({
	primaryColor: "teal",
	primaryShade: 8,
	defaultRadius: "sm",
	fontFamily: "var(--desk-font)",
	headings: {
		fontFamily: "var(--desk-font)",
		fontWeight: "600",
		sizes: {
			h1: { fontSize: "var(--desk-title)", lineHeight: "1.25" },
			h2: { fontSize: "1.125rem", lineHeight: "1.4" },
			h3: { fontSize: "1rem", lineHeight: "1.4" },
		},
	},
	components: {
		Group: { defaultProps: { gap: "sm" } },
		Stack: { defaultProps: { gap: "sm" } },
		Text: { defaultProps: { size: "sm" } },
	},
});

// On /sign-in the fragment is a credential, never a workspace route.
const confirming = window.location.pathname === "/sign-in";
const page = confirming ? (
	<ProjectDesk homeHref="/">
		<ConfirmSignIn />
	</ProjectDesk>
) : (
	<RouterProvider router={createWorkspaceRouter()} />
);
createRoot(root).render(
	<StrictMode>
		<MantineProvider theme={theme}>
			<QueryClientProvider client={queries}>
				<BackendContext.Provider value={services}>
					{page}
				</BackendContext.Provider>
			</QueryClientProvider>
		</MantineProvider>
	</StrictMode>,
);
