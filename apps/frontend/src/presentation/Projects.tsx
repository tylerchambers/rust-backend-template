import {
	Alert,
	Button,
	Group,
	Paper,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import {
	useInfiniteQuery,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";
import { useBackend } from "../bootstrap/context";
import {
	createProject,
	isVersionConflict,
	isWorkspaceUnavailableToUser,
	type Project,
	projectDetail,
	projectList,
	renameProject,
	workspaceError,
	workspaceName,
} from "../infrastructure/workspace";

function CreateProject({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const client = useBackend();
	const cache = useQueryClient();
	const [name, setName] = useState("");
	const parsed = workspaceName.safeParse(name);
	const create = useMutation({
		mutationFn: (newName: string) =>
			createProject(client, organizationId, newName),
		onSuccess: async () => {
			setName("");
			await cache.invalidateQueries({
				queryKey: projectList(client, organizationId).queryKey,
			});
		},
	});
	return (
		<Paper component="section" withBorder p="lg">
			<form
				onSubmit={(event) => {
					event.preventDefault();
					if (parsed.success && !create.isPending) create.mutate(parsed.data);
				}}
			>
				<Stack gap="sm">
					<Title order={2} size="h3">
						Create a project
					</Title>
					<TextInput
						label="Project name"
						required
						disabled={create.isPending}
						value={name}
						error={
							name && !parsed.success
								? parsed.error.issues[0]?.message
								: undefined
						}
						onChange={(event) => {
							setName(event.currentTarget.value);
							create.reset();
						}}
					/>
					{create.isError && (
						<Alert title="Project not created" color="red">
							{workspaceError(create.error)}
						</Alert>
					)}
					{create.isSuccess && (
						<Text role="status" size="sm">
							Created {create.data.name}.
						</Text>
					)}
					<Button
						type="submit"
						style={{ alignSelf: "start" }}
						disabled={!parsed.success}
						loading={create.isPending}
					>
						Create project
					</Button>
				</Stack>
			</form>
		</Paper>
	);
}

function RenameProject({
	project,
	onClose,
	onSave,
	onRefresh,
}: {
	project: Project;
	onClose: () => void;
	onSave: (updated: Project) => void;
	onRefresh: (updated: Project) => void;
}): React.JSX.Element {
	const client = useBackend();
	// Preserve the version the user started editing, even if the list refetches.
	const [base, setBase] = useState(project);
	const [name, setName] = useState(project.name);
	const parsed = workspaceName.safeParse(name);
	const rename = useMutation({
		mutationFn: (newName: string) => renameProject(client, base, newName),
	});
	const conflict = isVersionConflict(rename.error);
	const latest = useQuery({
		...projectDetail(client, project.organizationId, project.id),
		enabled: conflict,
		staleTime: 0,
	});
	return (
		<form
			onSubmit={(event) => {
				event.preventDefault();
				if (
					!conflict &&
					parsed.success &&
					!rename.isPending &&
					name !== base.name
				) {
					rename.mutate(parsed.data, { onSuccess: onSave });
				}
			}}
		>
			<Stack gap="sm">
				<TextInput
					label="New project name"
					description={`Editing version ${base.version}: ${base.name}`}
					required
					value={name}
					disabled={rename.isPending}
					error={
						name && !parsed.success
							? parsed.error.issues[0]?.message
							: undefined
					}
					onChange={(event) => setName(event.currentTarget.value)}
				/>
				{conflict ? (
					<Alert
						title="This project changed while you were editing"
						color="yellow"
					>
						<Stack gap="xs">
							<Text size="sm">
								Your draft was not saved. Compare it with the current name
								before trying again.
							</Text>
							{latest.isFetching ? (
								<Text role="status" size="sm">
									Loading the latest version…
								</Text>
							) : latest.isError ? (
								<>
									<Text size="sm">{workspaceError(latest.error)}</Text>
									<Button
										variant="light"
										size="xs"
										style={{ alignSelf: "start" }}
										onClick={() => void latest.refetch()}
									>
										Retry loading version
									</Button>
								</>
							) : (
								latest.data && (
									<>
										<Text size="sm">
											Current name (version {latest.data.version}):{" "}
											<strong>{latest.data.name}</strong>
										</Text>
										<Button
											variant="light"
											size="xs"
											style={{ alignSelf: "start" }}
											onClick={() => {
												setBase(latest.data);
												onRefresh(latest.data);
												rename.reset();
											}}
										>
											Continue with this version
										</Button>
									</>
								)
							)}
						</Stack>
					</Alert>
				) : (
					rename.isError && (
						<Alert title="Name not saved" color="red">
							{workspaceError(rename.error)}
						</Alert>
					)
				)}
				<Group gap="xs">
					<Button
						type="submit"
						size="xs"
						loading={rename.isPending}
						disabled={conflict || !parsed.success || name === base.name}
					>
						Save name
					</Button>
					<Button
						type="button"
						variant="subtle"
						size="xs"
						disabled={rename.isPending}
						onClick={() => {
							if (conflict && latest.isSuccess && !latest.isFetching)
								onRefresh(latest.data);
							onClose();
						}}
					>
						Cancel
					</Button>
				</Group>
			</Stack>
		</form>
	);
}

function ProjectCard({
	project,
	canWrite,
	onUpdate,
}: {
	project: Project;
	canWrite: boolean;
	onUpdate: (updated: Project) => void;
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
						onRefresh={onUpdate}
						onClose={() => setEditing(false)}
						onSave={(updated) => {
							onUpdate(updated);
							setNotice(`Renamed project to ${updated.name}.`);
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
	const cache = useQueryClient();
	const options = projectList(client, organizationId);
	const projects = useInfiniteQuery(options);
	const rows = projects.data?.pages.flat();
	function updateProject(updated: Project): void {
		cache.setQueryData(
			options.queryKey,
			(old) =>
				old && {
					...old,
					pages: old.pages.map((page) =>
						page.map((item) =>
							item.id === updated.id && item.version <= updated.version
								? updated
								: item,
						),
					),
				},
		);
	}
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
										onUpdate={updateProject}
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
