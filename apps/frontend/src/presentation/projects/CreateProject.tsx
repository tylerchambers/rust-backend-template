import {
	Alert,
	Button,
	Paper,
	Stack,
	Text,
	TextInput,
	Title,
} from "@mantine/core";
import { useMutation, useQueryClient } from "@tanstack/react-query";
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
