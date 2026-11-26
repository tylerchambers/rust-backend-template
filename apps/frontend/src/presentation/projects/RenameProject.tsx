import { Alert, Button, Group, Stack, Text, TextInput } from "@mantine/core";
import type { Project } from "../../infrastructure/projects";
import { useProjectRename } from "./useProjectRename";

function focusName(input: HTMLInputElement | null): void {
	input?.focus();
}

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
			className="entry-form"
			aria-label={`Rename ${project.name}`}
			onSubmit={(event) => {
				event.preventDefault();
				edit.save();
			}}
		>
			<Stack gap="sm">
				<TextInput
					aria-label="New project name"
					ref={focusName}
					size="lg"
					description={
						edit.base.version !== project.version
							? `Editing version ${edit.base.version}: ${edit.base.name}`
							: undefined
					}
					required
					value={edit.name}
					disabled={edit.isSaving}
					error={edit.nameError}
					onChange={(event) => edit.setName(event.currentTarget.value)}
				/>
				{conflict && (
					<Alert title="Project changed. Name not saved." color="yellow">
						<Stack gap="sm">
							{conflict.kind === "pending" && (
								<Text role="status" size="sm">
									{conflict.fetching
										? "Loading the latest version…"
										: "Waiting for the latest version…"}
								</Text>
							)}
							{conflict.kind === "failed" && (
								<>
									<Text size="sm">{conflict.message}</Text>
									<Button
										variant="default"
										className="fit-action"
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
										variant="default"
										className="fit-action"
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
				<Group gap="sm">
					<Button
						type="submit"
						loading={edit.isSaving}
						disabled={!edit.canSave}
					>
						Save name
					</Button>
					<Button
						type="button"
						variant="default"
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
