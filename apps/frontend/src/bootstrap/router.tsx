import { Stack, Title } from "@mantine/core";
import {
	createHashHistory,
	createRootRoute,
	createRoute,
	createRouter,
	Link,
	Outlet,
	type Router,
} from "@tanstack/react-router";
import type { JSX } from "react";
import { z } from "zod";
import { Organizations } from "../presentation/Organizations";
import { OrganizationWorkspace } from "../presentation/OrganizationWorkspace";
import { SessionGate } from "../presentation/SessionGate";

function NotFound(): JSX.Element {
	return (
		<Stack align="flex-start">
			<Title order={1}>Page not found</Title>
			<Link to="/">Back to organizations</Link>
		</Stack>
	);
}

const rootRoute = createRootRoute({
	component: () => (
		<SessionGate>
			<Outlet />
		</SessionGate>
	),
	notFoundComponent: NotFound,
});
const home = createRoute({
	getParentRoute: () => rootRoute,
	path: "/",
	component: Organizations,
});

const organizationIdSchema = z.uuid();
const organization = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/$organizationId",
	component: function OrganizationRoute() {
		const { organizationId } = organization.useParams();
		const id = organizationIdSchema.safeParse(organizationId.toLowerCase());
		return (
			<OrganizationWorkspace
				key={organizationId}
				organizationId={id.success ? id.data : null}
			/>
		);
	},
});

const routeTree = rootRoute.addChildren([home, organization]);
type WorkspaceRouter = Router<typeof routeTree>;

export function createWorkspaceRouter(): WorkspaceRouter {
	return createRouter({ routeTree, history: createHashHistory() });
}

declare module "@tanstack/react-router" {
	interface Register {
		router: WorkspaceRouter;
	}
}
