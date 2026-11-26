import {
	ActionIcon,
	Alert,
	Button,
	Group,
	Stack,
	Tabs,
	Text,
	Title,
} from "@mantine/core";
import {
	type InfiniteData,
	type UseInfiniteQueryResult,
	useInfiniteQuery,
} from "@tanstack/react-query";
import { getRouteApi, Link } from "@tanstack/react-router";
import { z } from "zod";
import { useBackend } from "../application/backend";
import { roleLabel } from "../domain/workspace";
import {
	type Organization,
	organizationList,
} from "../infrastructure/organizations";
import { workspaceError } from "../infrastructure/workspaceResponse";
import { AuditEvents } from "./AuditEvents";
import { Invitations } from "./Invitations";
import { InviteMember } from "./InviteMember";
import { Members } from "./Members";
import { Projects } from "./Projects";
import { CreateProject } from "./projects/CreateProject";
import { WorkspaceBreadcrumbs } from "./WorkspaceBreadcrumbs";

type OrganizationPage =
	| "workspace"
	| "settings"
	| "new-project"
	| "new-invitation";
const organizationIdSchema = z.uuid();

function OrganizationView({
	organizationId,
	page,
}: {
	organizationId: string;
	page: OrganizationPage;
}): React.JSX.Element {
	const organizations = useInfiniteQuery(organizationList(useBackend()));
	const org = organizations.data?.pages
		.flat()
		.find((row) => row.id === organizationId);
	const labels = {
		workspace: org?.name ?? "Organization",
		settings: "Settings",
		"new-project": "New project",
		"new-invitation": "Invite member",
	};
	return (
		<Stack gap="lg">
			<WorkspaceBreadcrumbs
				current={labels[page]}
				organizationId={page === "workspace" ? undefined : organizationId}
				organizationName={org?.name}
				viaSettings={page === "new-invitation"}
			/>
			<OrganizationContent organizations={organizations} org={org} page={page} />
		</Stack>
	);
}

function OrganizationContent({
	organizations,
	org,
	page,
}: {
	organizations: UseInfiniteQueryResult<InfiniteData<Organization[]>>;
	org: Organization | undefined;
	page: OrganizationPage;
}): React.JSX.Element {
	if (organizations.isPending)
		return <Text role="status">Loading organization…</Text>;
	if (organizations.isError)
		return (
			<Alert color="red" title="Could not load organization">
				<Text>{workspaceError(organizations.error)}</Text>
				<Button
					variant="default"
					mt="sm"
					onClick={() =>
						void (organizations.isFetchNextPageError
							? organizations.fetchNextPage()
							: organizations.refetch())
					}
				>
					Retry
				</Button>
			</Alert>
		);
	if (!org && organizations.hasNextPage)
		return (
			<Stack align="start">
				<Text c="dimmed">Organization not found on this page.</Text>
				<Button
					variant="default"
					loading={organizations.isFetchingNextPage}
					onClick={() => void organizations.fetchNextPage()}
				>
					Search next page
				</Button>
			</Stack>
		);
	if (!org) return <Title order={1}>Organization not available</Title>;
	const organizationId = org.id;
	if (page === "new-project") return <CreateProject organizationId={org.id} />;
	if (page === "new-invitation") {
		if (org.isAdmin) return <InviteMember organizationId={org.id} />;
		return <Title order={1}>Only admins can invite members</Title>;
	}
	if (page === "settings") return <OrganizationSettings org={org} />;
	return (
		<>
			<Group gap="xs">
				<Title order={1}>{org.name}</Title>
				<ActionIcon
					variant="subtle"
					color="gray"
					aria-label="Organization settings"
					title="Organization settings"
					renderRoot={(props) => (
						<Link
							to="/organizations/$organizationId/settings"
							params={{ organizationId }}
							{...props}
						/>
					)}
				>
					<svg
						aria-hidden="true"
						width="20"
						height="20"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						strokeWidth="1.7"
						strokeLinejoin="round"
					>
						<path d="m9.5 3-.6 2.3-1.6.9L5 5.6 2.5 9.9l1.7 1.6v1.9l-1.7 1.5L5 19.2l2.3-.6 1.6.9.6 2.3h5l.6-2.3 1.6-.9 2.3.6 2.5-4.3-1.7-1.5v-1.9l1.7-1.6L19 5.6l-2.3.6-1.6-.9-.6-2.3z" />
						<circle cx="12" cy="12.4" r="3.1" />
					</svg>
				</ActionIcon>
				<Text c="dimmed">{roleLabel(org.isAdmin)}</Text>
				<Button
					ml="auto"
					renderRoot={(props) => (
						<Link
							to="/organizations/$organizationId/projects/new"
							params={{ organizationId }}
							{...props}
						/>
					)}
				>
					New project
				</Button>
			</Group>
			<Projects organizationId={org.id} />
		</>
	);
}

const settingsRoute = getRouteApi("/organizations/$organizationId/settings");

function OrganizationSettings({
	org,
}: {
	org: Organization;
}): React.JSX.Element {
	const { tab } = settingsRoute.useSearch();
	const navigate = settingsRoute.useNavigate();
	return (
		<Stack gap="lg">
			<Title order={1}>{org.name} settings</Title>
			<Tabs
				value={org.isAdmin && tab === "audit" ? "audit" : "members"}
				onChange={(next) => {
					void navigate({
						search: next === "audit" ? { tab: "audit" } : {},
						replace: true,
					});
				}}
				keepMounted={false}
			>
				<Tabs.List aria-label="Organization settings">
					<Tabs.Tab value="members">Members</Tabs.Tab>
					{org.isAdmin && <Tabs.Tab value="audit">Audit events</Tabs.Tab>}
				</Tabs.List>
				<Tabs.Panel value="members" pt="lg">
					<Stack gap="lg">
						<Members organizationId={org.id} isAdmin={org.isAdmin} />
						{org.isAdmin && (
							<Group justify="flex-end">
								<Button
									renderRoot={(props) => (
										<Link
											to="/organizations/$organizationId/invitations/new"
											params={{ organizationId: org.id }}
											{...props}
										/>
									)}
								>
									Invite member
								</Button>
							</Group>
						)}
						{org.isAdmin && <Invitations organizationId={org.id} />}
					</Stack>
				</Tabs.Panel>
				{org.isAdmin && (
					<Tabs.Panel value="audit" pt="lg">
						<AuditEvents organizationId={org.id} />
					</Tabs.Panel>
				)}
			</Tabs>
		</Stack>
	);
}

export function OrganizationWorkspace({
	organizationId,
	page = "workspace",
}: {
	organizationId: string;
	page?: OrganizationPage;
}): React.JSX.Element {
	const id = organizationIdSchema.safeParse(organizationId.toLowerCase());
	if (id.success) return <OrganizationView organizationId={id.data} page={page} />;
	return (
		<Stack gap="lg">
			<WorkspaceBreadcrumbs current="Invalid organization link" />
			<Title order={1}>Invalid organization link</Title>
		</Stack>
	);
}
