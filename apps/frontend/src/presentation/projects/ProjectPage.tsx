import {
	ActionIcon,
	Alert,
	Button,
	Group,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { type UseQueryResult, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { z } from "zod";
import { useBackend } from "../../application/backend";
import { organizationList } from "../../infrastructure/organizations";
import { type Project, projectDetail } from "../../infrastructure/projects";
import { workspaceError } from "../../infrastructure/workspaceResponse";
import { WorkspaceBreadcrumbs } from "../WorkspaceBreadcrumbs";
import { RenameProject } from "./RenameProject";

type ProjectParams = { organizationId: string; projectId: string };
const uuid = z.uuid();

function ProjectDetails({
	organizationId,
	projectId,
}: ProjectParams): React.JSX.Element {
	const backend = useBackend();
	const project = useQuery(projectDetail(backend, organizationId, projectId));
	const organizations = useQueryClient().getQueryData(organizationList(backend).queryKey);
	const org = organizations?.pages.flat().find((row) => row.id === organizationId);
	return (
		<Stack gap="lg">
			<WorkspaceBreadcrumbs
				current={project.data?.name ?? "Project"}
				organizationId={organizationId}
				organizationName={org?.name}
			/>
			<ProjectContent project={project} />
		</Stack>
	);
}

function ProjectContent({ project }: { project: UseQueryResult<Project> }): React.JSX.Element {
	const [editing, setEditing] = useState(false);
	if (project.isPending) return <Text role="status">Loading project…</Text>;
	if (project.isError) {
		return (
			<Alert title="Could not load project" color="red">
				<Text>{workspaceError(project.error)}</Text>
				<Button
					variant="default"
					mt="sm"
					loading={project.isFetching}
					onClick={() => void project.refetch()}
				>
					Retry
				</Button>
			</Alert>
		);
	}
	return (
		<Stack align="start">
			{editing ? (
				<RenameProject
					project={project.data}
					onCancel={() => setEditing(false)}
					onSaved={() => setEditing(false)}
				/>
			) : (
				<Group gap="xs" wrap="nowrap" maw="100%">
					<Title order={1}>{project.data.name}</Title>
					<ActionIcon
						variant="subtle"
						color="gray"
						aria-label="Rename project"
						title="Rename project"
						onClick={() => setEditing(true)}
					>
						<svg
							aria-hidden="true"
							width="18"
							height="18"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							strokeWidth="1.75"
							strokeLinejoin="round"
						>
							<path d="m4 20 4.5-1 12-12a2.5 2.5 0 0 0-3.5-3.5l-12 12L4 20Z M15 5.5 18.5 9" />
						</svg>
					</ActionIcon>
				</Group>
			)}
			<Text>
				{project.data.archived ? "Archived" : "Active"} · Version{" "}
				{project.data.version}
			</Text>
		</Stack>
	);
}

export function ProjectPage({
	organizationId,
	projectId,
}: ProjectParams): React.JSX.Element {
	const org = uuid.safeParse(organizationId.toLowerCase());
	const id = uuid.safeParse(projectId.toLowerCase());
	if (org.success && id.success) {
		return <ProjectDetails key={`${org.data}:${id.data}`} organizationId={org.data} projectId={id.data} />;
	}
	return (
		<Stack gap="lg">
			<WorkspaceBreadcrumbs current="Invalid project link" organizationId={org.data} />
			<Title order={1}>Invalid project link</Title>
		</Stack>
	);
}
