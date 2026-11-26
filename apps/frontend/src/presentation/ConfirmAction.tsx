import { Alert, Button, Group, Modal, Stack, Text } from "@mantine/core";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useId, useState } from "react";
import { workspaceError } from "../infrastructure/workspaceResponse";

type Props = {
	destructive?: boolean;
	label: string;
	description: string;
	submit: () => Promise<void>;
	onSuccess: () => void;
	onCancel: () => void;
};

export function ConfirmAction({
	destructive,
	label,
	description,
	submit,
	onSuccess,
	onCancel,
}: Props): React.JSX.Element {
	const descriptionId = useId();
	const [opener] = useState(() => document.activeElement);
	// The dialog unmounts on close rather than toggling `opened`.
	useEffect(
		() => () => {
			if (
				opener instanceof HTMLElement &&
				opener.isConnected &&
				document.activeElement === document.body
			) {
				opener.focus({ preventScroll: true });
			}
		},
		[opener],
	);
	const confirm = useMutation({
		mutationFn: submit,
		networkMode: "always",
		retry: false,
		onSuccess,
	});
	return (
		<Modal
			opened
			onClose={onCancel}
			title={label}
			withCloseButton={false}
			aria-describedby={descriptionId}
			closeOnEscape={!confirm.isPending}
			closeOnClickOutside={!confirm.isPending}
		>
			<Stack gap="sm" style={{ overflowWrap: "anywhere" }}>
				<Text id={descriptionId}>{description}</Text>
				{confirm.isError && (
					<Alert color="red" title="Action failed">
						{workspaceError(confirm.error)}
					</Alert>
				)}
				<Group gap="sm">
					<Button
						variant="default"
						data-autofocus
						disabled={confirm.isPending}
						onClick={onCancel}
					>
						Cancel
					</Button>
					<Button
						{...(destructive && { color: "red" })}
						loading={confirm.isPending}
						onClick={() => confirm.mutate()}
					>
						{label}
					</Button>
				</Group>
			</Stack>
		</Modal>
	);
}
