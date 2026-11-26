import { Alert, Button, Group, Paper, Stack, Text, Title } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useBackend } from "../application/backend";
import { type Project, projectList } from "../infrastructure/projects";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";
import { CreateProject } from "./projects/CreateProject";
import { RenameProject } from "./projects/RenameProject";

function ProjectCard({
	project,
	canWrite,
}: {
	project: Project;
	canWrite: boolean;
}): React.JSX.Element {
	const [editing, setEditing] = useState(false);
	const [notice, setNotice] = useState("");
	return (
		<Paper component="li" withBorder p="md">
			<Stack gap="sm">
				<Group justify="space-between">
					<div>
						<Title order={2} size="h4" style={{ overflowWrap: "anywhere" }}>
							{project.name}
						</Title>
						<Text size="xs" c="dimmed">
							{project.archived ? "Archived" : "Active"} · Version{" "}
							{project.version}
						</Text>
					</div>
					{canWrite && !editing && (
						<Button
							size="xs"
							variant="light"
							aria-label={`Rename ${project.name}`}
							onClick={() => {
								setNotice("");
								setEditing(true);
							}}
						>
							Rename
						</Button>
					)}
				</Group>
				{canWrite && editing && (
					<RenameProject
						project={project}
						onCancel={() => setEditing(false)}
						onSaved={(name) => {
							setNotice(`Renamed project to ${name}.`);
							setEditing(false);
						}}
					/>
				)}
				{notice && (
					<Text role="status" size="sm">
						{notice}
					</Text>
				)}
			</Stack>
		</Paper>
	);
}

export function Projects({
	organizationId,
	canWrite,
}: {
	organizationId: string;
	canWrite: boolean;
}): React.JSX.Element {
	const client = useBackend();
	const projects = useInfiniteQuery(projectList(client, organizationId));
	const rows = projects.data?.pages.flat();
	return (
		<Stack gap="xl">
			<section
				aria-label="Organization projects"
				aria-busy={projects.isFetching}
			>
				{projects.isPending && <Text role="status">Loading projects…</Text>}
				{projects.isError && (
					<Alert
						title={
							projects.isFetchNextPageError
								? "Could not load more projects"
								: "Could not load projects"
						}
						color="red"
					>
						<Text size="sm">{workspaceError(projects.error)}</Text>
						<Button
							variant="light"
							size="xs"
							mt="sm"
							loading={projects.isFetching}
							onClick={() => {
								if (projects.isFetchNextPageError)
									void projects.fetchNextPage();
								else void projects.refetch();
							}}
						>
							Retry
						</Button>
					</Alert>
				)}
				{rows && !isWorkspaceUnavailableToUser(projects.error) && (
					<Stack>
						{rows.length === 0 ? (
							<Paper withBorder p="lg">
								<Title order={2} size="h4">
									No projects yet
								</Title>
								<Text size="sm" c="dimmed">
									{canWrite
										? "Create the first project for this organization below."
										: "Projects created by your team will appear here."}
								</Text>
							</Paper>
						) : (
							<Stack
								component="ul"
								p={0}
								m={0}
								gap="xs"
								style={{ listStyle: "none" }}
							>
								{rows.map((project) => (
									<ProjectCard
										key={project.id}
										project={project}
										canWrite={canWrite}
									/>
								))}
							</Stack>
						)}
						{projects.hasNextPage && !projects.isFetchNextPageError && (
							<Button
								variant="light"
								style={{ alignSelf: "start" }}
								disabled={projects.isFetching}
								loading={projects.isFetchingNextPage}
								onClick={() => void projects.fetchNextPage()}
							>
								Load more projects
							</Button>
						)}
					</Stack>
				)}
			</section>
			{canWrite && projects.isSuccess && (
				<CreateProject organizationId={organizationId} />
			)}
			{!canWrite && (
				<Text c="dimmed" size="sm">
					You have view-only access to this organization.
				</Text>
			)}
		</Stack>
	);
}
