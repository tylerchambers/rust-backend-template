import { Alert, Button, Group, Stack, Tabs, Text, Title } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useBackend } from "../application/backend";
import { canManageTeam, canWriteProjects } from "../domain/workspace";
import {
	type Organization,
	organizationList,
} from "../infrastructure/organizations";
import { workspaceError } from "../infrastructure/workspaceResponse";
import { AuditEvents } from "./AuditEvents";
import { Invitations } from "./Invitations";
import { Members } from "./Members";
import { Projects } from "./Projects";

function OrganizationView({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const organizations = useInfiniteQuery(organizationList(useBackend()));
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
	const org = organizations.data.pages
		.flat()
		.find((row) => row.id === organizationId);
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
	return <OrganizationTabs org={org} />;
}

function OrganizationTabs({ org }: { org: Organization }): React.JSX.Element {
	const managesTeam = canManageTeam(org.role);
	return (
		<>
			<Group gap="sm">
				<Title order={1}>{org.name}</Title>
				<Text c="dimmed">{org.role}</Text>
			</Group>
			<Tabs key={org.role} defaultValue="projects" keepMounted={false}>
				<Tabs.List aria-label="Organization sections">
					<Tabs.Tab value="projects">Projects</Tabs.Tab>
					<Tabs.Tab value="members">Members</Tabs.Tab>
					{managesTeam && <Tabs.Tab value="invitations">Invitations</Tabs.Tab>}
					{managesTeam && <Tabs.Tab value="audit">Audit events</Tabs.Tab>}
				</Tabs.List>
				<Tabs.Panel value="projects" pt="lg">
					<Projects
						organizationId={org.id}
						canWrite={canWriteProjects(org.role)}
					/>
				</Tabs.Panel>
				<Tabs.Panel value="members" pt="lg">
					<Members organizationId={org.id} role={org.role} />
				</Tabs.Panel>
				{managesTeam && (
					<Tabs.Panel value="invitations" pt="lg">
						<Invitations organizationId={org.id} role={org.role} />
					</Tabs.Panel>
				)}
				{managesTeam && (
					<Tabs.Panel value="audit" pt="lg">
						<AuditEvents organizationId={org.id} />
					</Tabs.Panel>
				)}
			</Tabs>
		</>
	);
}

export function OrganizationWorkspace({
	organizationId,
}: {
	organizationId: string | null;
}): React.JSX.Element {
	return (
		<Stack gap="lg">
			<nav aria-label="Organization navigation">
				<Link to="/">Organizations</Link>
			</nav>
			{organizationId === null ? (
				<Title order={1}>Invalid organization link</Title>
			) : (
				<OrganizationView organizationId={organizationId} />
			)}
		</Stack>
	);
}
