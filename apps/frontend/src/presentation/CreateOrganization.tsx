import { Alert, Button, Group, Stack, TextInput, Title } from "@mantine/core";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
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
	const navigate = useNavigate();
	const [name, setName] = useState("");
	const parsed = workspaceName.safeParse(name);
	const create = useMutation({
		mutationFn: (newName: string) => createOrganization(client, newName),
		onSuccess: () => cache.invalidateQueries({ queryKey: organizationListKey }),
	});
	return (
		<form
			className="entry-form"
			onSubmit={(event) => {
				event.preventDefault();
				if (parsed.success && !create.isPending) {
					create.mutate(parsed.data, {
						onSuccess: () => void navigate({ to: "/", replace: true }),
					});
				}
			}}
		>
			<Stack>
				<Title order={1}>New organization</Title>
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
				<Group>
					<Button
						type="submit"
						loading={create.isPending}
						disabled={!parsed.success}
					>
						Create organization
					</Button>
					{!create.isPending && <Link to="/">Cancel</Link>}
				</Group>
			</Stack>
		</form>
	);
}
