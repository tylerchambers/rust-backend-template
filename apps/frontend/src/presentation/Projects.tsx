import { Alert, Button, Stack, Text, Title } from "@mantine/core";
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

function ProjectRow({
	project,
	canWrite,
}: {
	project: Project;
	canWrite: boolean;
}): React.JSX.Element {
	const [editing, setEditing] = useState(false);
	const [notice, setNotice] = useState("");
	return (
		<li>
			<Stack gap="sm">
				<div className="record-row">
					<div>
						<Title order={2}>{project.name}</Title>
						<Text size="sm" c="dimmed">
							{project.archived ? "Archived" : "Active"} · Version{" "}
							{project.version}
						</Text>
					</div>
					{canWrite && !editing && (
						<Button
							variant="default"
							aria-label={`Rename ${project.name}`}
							onClick={() => {
								setNotice("");
								setEditing(true);
							}}
						>
							Rename
						</Button>
					)}
				</div>
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
		</li>
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
		<Stack gap="lg">
			{canWrite && projects.isSuccess && (
				<CreateProject organizationId={organizationId} />
			)}
			<Stack
				component="section"
				gap="sm"
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
							variant="default"
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
					<>
						{rows.length === 0 ? (
							<Text c="dimmed">No projects yet.</Text>
						) : (
							<ul className="record-list">
								{rows.map((project) => (
									<ProjectRow
										key={project.id}
										project={project}
										canWrite={canWrite}
									/>
								))}
							</ul>
						)}
						{projects.hasNextPage && !projects.isFetchNextPageError && (
							<Button
								variant="default"
								className="fit-action"
								disabled={projects.isFetching}
								loading={projects.isFetchingNextPage}
								onClick={() => void projects.fetchNextPage()}
							>
								Load more projects
							</Button>
						)}
					</>
				)}
			</Stack>
		</Stack>
	);
}
