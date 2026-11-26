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
import { Link } from "@tanstack/react-router";
import { useState } from "react";
import { useBackend } from "../application/backend";
import { workspaceName } from "../domain/workspace";
import {
	createOrganization,
	organizationListKey,
} from "../infrastructure/organizations";
import { workspaceError } from "../infrastructure/workspaceResponse";

export function CreateOrganization(): React.JSX.Element {
	const client = useBackend();
	const cache = useQueryClient();
	const [name, setName] = useState("");
	const parsed = workspaceName.safeParse(name);
	const create = useMutation({
		mutationFn: (newName: string) => createOrganization(client, newName),
		onSuccess: async () => {
			setName("");
			await cache.invalidateQueries({ queryKey: organizationListKey });
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
				<Stack align="stretch" gap="sm">
					<Title order={2} size="h3">
						Create an organization
					</Title>
					<TextInput
						label="Organization name"
						required
						value={name}
						disabled={create.isPending}
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
						<Alert color="red" title="Organization not created">
							{workspaceError(create.error)}
						</Alert>
					)}
					{create.isSuccess && (
						<Text role="status" size="sm">
							Organization created.{" "}
							<Link
								to="/organizations/$organizationId"
								params={{ organizationId: create.data }}
							>
								Open {create.variables}
							</Link>
						</Text>
					)}
					<Button
						type="submit"
						loading={create.isPending}
						disabled={!parsed.success}
						style={{ alignSelf: "start" }}
					>
						Create organization
					</Button>
				</Stack>
			</form>
		</Paper>
	);
}
