import { Alert, Button, Stack, Table, Text } from "@mantine/core";
import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useBackend } from "../application/backend";
import { projectList } from "../infrastructure/projects";
import {
	isWorkspaceUnavailableToUser,
	workspaceError,
} from "../infrastructure/workspaceResponse";

export function Projects({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const client = useBackend();
	const projects = useInfiniteQuery(projectList(client, organizationId));
	const rows = projects.data?.pages.flat();
	return (
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
							if (projects.isFetchNextPageError) void projects.fetchNextPage();
							else void projects.refetch();
						}}
					>
						Retry
					</Button>
				</Alert>
			)}
			{rows && !isWorkspaceUnavailableToUser(projects.error) && (
				<>
					<Table aria-label="Projects">
						<Table.Thead>
							<Table.Tr>
								<Table.Th scope="col">Project</Table.Th>
								<Table.Th scope="col">Status</Table.Th>
								<Table.Th scope="col">Version</Table.Th>
							</Table.Tr>
						</Table.Thead>
						<Table.Tbody>
							{rows.length === 0 && (
								<Table.Tr>
									<Table.Td colSpan={3}>No projects yet.</Table.Td>
								</Table.Tr>
							)}
							{rows.map((project) => (
								<Table.Tr key={project.id}>
									<Table.Td>
										<Link
											to="/organizations/$organizationId/projects/$projectId"
											params={{ organizationId, projectId: project.id }}
										>
											{project.name}
										</Link>
									</Table.Td>
									<Table.Td>
										{project.archived ? "Archived" : "Active"}
									</Table.Td>
									<Table.Td>{project.version}</Table.Td>
								</Table.Tr>
							))}
						</Table.Tbody>
					</Table>
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
	);
}
