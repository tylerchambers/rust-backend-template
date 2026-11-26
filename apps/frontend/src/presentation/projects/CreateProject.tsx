import { Alert, Button, Group, Stack, TextInput, Title } from "@mantine/core";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useBackend } from "../../application/backend";
import { workspaceName } from "../../domain/workspace";
import { createProject, projectList } from "../../infrastructure/projects";
import { workspaceError } from "../../infrastructure/workspaceResponse";

export function CreateProject({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const client = useBackend();
	const cache = useQueryClient();
	const navigate = useNavigate();
	const projectsPage = {
		to: "/organizations/$organizationId",
		params: { organizationId },
	} as const;
	const [name, setName] = useState("");
	const parsed = workspaceName.safeParse(name);
	const create = useMutation({
		mutationFn: (newName: string) =>
			createProject(client, organizationId, newName),
		onSuccess: () =>
			cache.invalidateQueries({
				queryKey: projectList(client, organizationId).queryKey,
			}),
	});
	return (
		<form
			className="entry-form"
			aria-label="Create project"
			onSubmit={(event) => {
				event.preventDefault();
				if (parsed.success && !create.isPending) {
					create.mutate(parsed.data, {
						onSuccess: () => void navigate({ ...projectsPage, replace: true }),
					});
				}
			}}
		>
			<Stack gap="sm">
				<Title order={1}>New project</Title>
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
				<Group>
					<Button
						type="submit"
						disabled={!parsed.success}
						loading={create.isPending}
					>
						Create project
					</Button>
					{!create.isPending && <Link {...projectsPage}>Cancel</Link>}
				</Group>
			</Stack>
		</form>
	);
}
