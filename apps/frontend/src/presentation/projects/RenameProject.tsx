import { Alert, Button, Group, Stack, Text, TextInput } from "@mantine/core";
import type { Project } from "../../infrastructure/projects";
import { useProjectRename } from "./useProjectRename";

export function RenameProject({
	project,
	onSaved,
	onCancel,
}: {
	project: Project;
	onSaved: (name: string) => void;
	onCancel: () => void;
}): React.JSX.Element {
	const edit = useProjectRename(project, onSaved, onCancel);
	const conflict = edit.conflict;
	return (
		<form
			onSubmit={(event) => {
				event.preventDefault();
				edit.save();
			}}
		>
			<Stack gap="sm">
				<TextInput
					label="New project name"
					description={`Editing version ${edit.base.version}: ${edit.base.name}`}
					required
					value={edit.name}
					disabled={edit.isSaving}
					error={edit.nameError}
					onChange={(event) => edit.setName(event.currentTarget.value)}
				/>
				{conflict && (
					<Alert
						title="This project changed while you were editing"
						color="yellow"
					>
						<Stack gap="xs">
							<Text size="sm">
								Your draft was not saved. Compare it with the current name
								before trying again.
							</Text>
							{conflict.kind === "pending" && conflict.fetching && (
								<Text role="status" size="sm">
									Loading the latest version…
								</Text>
							)}
							{conflict.kind === "failed" && (
								<>
									<Text size="sm">{conflict.message}</Text>
									<Button
										variant="light"
										size="xs"
										style={{ alignSelf: "start" }}
										onClick={conflict.retry}
									>
										Retry loading version
									</Button>
								</>
							)}
							{conflict.kind === "ready" && (
								<>
									<Text size="sm">
										Current name (version {conflict.version}):{" "}
										<strong>{conflict.name}</strong>
									</Text>
									<Button
										variant="light"
										size="xs"
										style={{ alignSelf: "start" }}
										onClick={conflict.accept}
									>
										Continue with this version
									</Button>
								</>
							)}
						</Stack>
					</Alert>
				)}
				{edit.saveError !== null && (
					<Alert title="Name not saved" color="red">
						{edit.saveError}
					</Alert>
				)}
				<Group gap="xs">
					<Button
						type="submit"
						size="xs"
						loading={edit.isSaving}
						disabled={!edit.canSave}
					>
						Save name
					</Button>
					<Button
						type="button"
						variant="subtle"
						size="xs"
						disabled={edit.isSaving}
						onClick={edit.cancel}
					>
						Cancel
					</Button>
				</Group>
			</Stack>
		</form>
	);
}
