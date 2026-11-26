import {
	Alert,
	Badge,
	Button,
	Group,
	Stack,
	Tabs,
	Text,
	Title,
} from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { z } from "zod";
import { useBackend } from "../bootstrap/context";
import { organizationList, workspaceError } from "../infrastructure/workspace";
import { AuditEvents } from "./AuditEvents";
import { Invitations } from "./Invitations";
import { Members } from "./Members";
import { Projects } from "./Projects";

function OrganizationView({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const client = useBackend();
	const id = z.uuid().safeParse(organizationId.toLowerCase());
	const organizations = useInfiniteQuery({
		...organizationList(client),
		enabled: id.success,
	});
	if (!id.success) return <Title order={1}>Invalid organization link</Title>;
	if (organizations.isPending)
		return <Text role="status">Loading organization…</Text>;
	if (organizations.isError)
		return (
			<Alert color="red" title="Could not load organization">
				<Text>{workspaceError(organizations.error)}</Text>
				<Button
					variant="light"
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
	const org = organizations.data.pages.flat().find((row) => row.id === id.data);
	if (!org && organizations.hasNextPage)
		return (
			<Stack align="start">
				<Text>This organization is not on the loaded pages.</Text>
				<Button
					variant="light"
					loading={organizations.isFetchingNextPage}
					onClick={() => void organizations.fetchNextPage()}
				>
					Search next page of organizations
				</Button>
			</Stack>
		);
	if (!org) return <Title order={1}>Organization not available</Title>;
	const managesTeam = org.role === "owner" || org.role === "admin";
	return (
		<>
			<Group gap="sm">
				<Title order={1} style={{ overflowWrap: "anywhere" }}>
					{org.name}
				</Title>
				<Badge variant="light">{org.role}</Badge>
			</Group>
			<Tabs key={org.role} defaultValue="projects" keepMounted={false}>
				<Tabs.List aria-label="Organization sections">
					<Tabs.Tab value="projects">Projects</Tabs.Tab>
					<Tabs.Tab value="members">Members</Tabs.Tab>
					{managesTeam && <Tabs.Tab value="invitations">Invitations</Tabs.Tab>}
					{managesTeam && <Tabs.Tab value="audit">Audit events</Tabs.Tab>}
				</Tabs.List>
				<Tabs.Panel value="projects" pt="lg">
					<Projects organizationId={org.id} canWrite={org.role !== "viewer"} />
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
	organizationId: string;
}): React.JSX.Element {
	return (
		<Stack gap="lg">
			<Link to="/">All organizations</Link>
			<OrganizationView organizationId={organizationId} />
		</Stack>
	);
}
