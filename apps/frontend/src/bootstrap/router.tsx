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
import { CreateOrganization } from "../presentation/CreateOrganization";
import { Organizations } from "../presentation/Organizations";
import { OrganizationWorkspace } from "../presentation/OrganizationWorkspace";
import { ProjectPage } from "../presentation/projects/ProjectPage";
import { SessionGate } from "../presentation/SessionGate";
import { Sessions } from "../presentation/Sessions";
import { WorkspaceBreadcrumbs } from "../presentation/WorkspaceBreadcrumbs";

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

const accountSettings = createRoute({
	getParentRoute: () => rootRoute,
	path: "/account/settings",
	component: () => (
		<Stack gap="lg">
			<Title order={1}>Account settings</Title>
			<Sessions />
		</Stack>
	),
});

const newOrganization = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/new",
	component: () => (
		<Stack gap="lg">
			<WorkspaceBreadcrumbs current="New organization" />
			<CreateOrganization />
		</Stack>
	),
});

const organization = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/$organizationId",
	component: function OrganizationRoute() {
		const { organizationId } = organization.useParams();
		return (
			<OrganizationWorkspace
				key={organizationId}
				organizationId={organizationId}
			/>
		);
	},
});
const organizationSettings = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/$organizationId/settings",
	validateSearch: (search): { tab?: "audit" } =>
		search.tab === "audit" ? { tab: "audit" } : {},
	component: function OrganizationSettingsRoute() {
		const { organizationId } = organizationSettings.useParams();
		return (
			<OrganizationWorkspace
				key={organizationId}
				organizationId={organizationId}
				page="settings"
			/>
		);
	},
});

const newProject = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/$organizationId/projects/new",
	component: function NewProjectRoute() {
		const { organizationId } = newProject.useParams();
		return (
			<OrganizationWorkspace
				key={organizationId}
				organizationId={organizationId}
				page="new-project"
			/>
		);
	},
});

const project = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/$organizationId/projects/$projectId",
	component: function ProjectRoute() {
		return <ProjectPage {...project.useParams()} />;
	},
});

const newInvitation = createRoute({
	getParentRoute: () => rootRoute,
	path: "/organizations/$organizationId/invitations/new",
	component: function NewInvitationRoute() {
		const { organizationId } = newInvitation.useParams();
		return (
			<OrganizationWorkspace
				key={organizationId}
				organizationId={organizationId}
				page="new-invitation"
			/>
		);
	},
});

const routeTree = rootRoute.addChildren([
	home,
	accountSettings,
	newOrganization,
	organization,
	organizationSettings,
	newProject,
	project,
	newInvitation,
]);
type WorkspaceRouter = Router<typeof routeTree>;

export function createWorkspaceRouter(): WorkspaceRouter {
	return createRouter({ routeTree, history: createHashHistory() });
}

declare module "@tanstack/react-router" {
	interface Register {
		router: WorkspaceRouter;
	}
}
