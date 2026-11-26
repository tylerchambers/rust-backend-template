import {
	Anchor,
	Box,
	Container,
	Group,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import {
	createHashHistory,
	createRootRoute,
	createRoute,
	createRouter,
	Link,
	Outlet,
} from "@tanstack/react-router";
import type { JSX } from "react";
import { z } from "zod";
import { Organizations } from "../presentation/Organizations";
import { OrganizationWorkspace } from "../presentation/OrganizationWorkspace";
import { SessionGate } from "../presentation/SessionGate";

function Layout(): JSX.Element {
	return (
		<>
			<Box component="header" className="masthead">
				<Container size="md">
					<Group justify="space-between" py="md">
						<Link to="/" className="wordmark">
							<span className="brand-mark" aria-hidden="true">
								P
							</span>
							Project desk
						</Link>
						<Anchor href="/openapi.json" c="gray.2" size="sm">
							API contract
						</Anchor>
					</Group>
				</Container>
			</Box>
			<Container component="main" size="md" py="xl">
				<SessionGate>
					<Outlet />
				</SessionGate>
			</Container>
		</>
	);
}

function NotFound(): JSX.Element {
	return (
		<Stack align="flex-start">
			<Title order={1}>Page not found</Title>
			<Text>This workspace link does not match a page.</Text>
			<Link to="/">Back to organizations</Link>
		</Stack>
	);
}

const rootRoute = createRootRoute({
	component: Layout,
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

export const router = createRouter({
	routeTree: rootRoute.addChildren([home, organization]),
	history: createHashHistory(),
});

declare module "@tanstack/react-router" {
	interface Register {
		router: typeof router;
	}
}
