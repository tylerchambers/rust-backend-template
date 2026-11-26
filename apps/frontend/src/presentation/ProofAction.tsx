import {
	Alert,
	Button,
	Group,
	Modal,
	Stack,
	Text,
	TextInput,
} from "@mantine/core";
import { useMutation } from "@tanstack/react-query";
import { type FormEvent, useEffect, useState } from "react";
import { useBackend } from "../application/backend";
import {
	requestTeamProof,
	type TeamProofRequest,
} from "../infrastructure/teamProof";
import { workspaceError } from "../infrastructure/workspaceResponse";

type Props = TeamProofRequest & {
	label: string;
	description: string;
	submit: (proof: string) => Promise<void>;
	onSuccess: () => void;
	onCancel: () => void;
};

function focusProof(input: HTMLInputElement | null): void {
	input?.focus();
}

export function ProofAction({
	organizationId,
	action,
	target,
	label,
	description,
	submit,
	onSuccess,
	onCancel,
}: Props): React.JSX.Element {
	const client = useBackend();
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
	const [submitting, setSubmitting] = useState(false);
	const [error, setError] = useState("");
	const request = useMutation({
		networkMode: "always",
		retry: false,
		mutationFn: () =>
			requestTeamProof(client, { organizationId, action, target }),
	});
	// Keep the bearer proof out of React Query's mutation cache and the URL.
	async function confirm(event: FormEvent<HTMLFormElement>): Promise<void> {
		event.preventDefault();
		if (submitting || !request.isSuccess) return;
		const form = event.currentTarget;
		const proof = new FormData(form).get("proof");
		if (typeof proof !== "string" || !proof.trim()) return;
		form.reset();
		setError("");
		setSubmitting(true);
		try {
			await submit(proof.trim());
		} catch (cause) {
			setError(workspaceError(cause));
			return;
		} finally {
			setSubmitting(false);
		}
		onSuccess();
	}
	const busy = submitting || request.isPending;
	const destructive =
		action === "remove_member" || action === "revoke_invitation";
	return (
		<Modal
			opened
			onClose={onCancel}
			title={label}
			withCloseButton={false}
			closeOnEscape={!busy}
			closeOnClickOutside={!busy}
		>
			<Stack gap="sm" style={{ overflowWrap: "anywhere" }}>
				<Text>{description}</Text>
				<div>
					<Text size="sm" c="dimmed">
						Organization: {organizationId}
					</Text>
					{target !== organizationId && (
						<Text size="sm" c="dimmed">
							Target: {target}
						</Text>
					)}
				</div>
				{request.isSuccess ? (
					<form onSubmit={confirm}>
						<Stack gap="sm">
							<Text size="sm" role="status">
								Open the email in this browser. Check the action and target, then
								choose "Confirm and reveal proof". Paste the proof within five
								minutes.
							</Text>
							<TextInput
								name="proof"
								label="Email proof"
								type="password"
								autoComplete="off"
								spellCheck={false}
								required
								disabled={submitting}
								ref={focusProof}
							/>
							{error && (
								<Alert color="red" title="Action not confirmed">
									{error} Request a new proof if yours has expired or been used.
								</Alert>
							)}
							<Group gap="sm">
								<Button
									type="submit"
									{...(destructive && { variant: "subtle", color: "red" })}
									loading={submitting}
								>
									{label}
								</Button>
								<Button
									type="button"
									variant="default"
									disabled={submitting}
									onClick={() => {
										setError("");
										request.mutate();
									}}
								>
									Email a new proof
								</Button>
								<Button
									type="button"
									variant="default"
									disabled={submitting}
									onClick={onCancel}
								>
									Cancel
								</Button>
							</Group>
						</Stack>
					</form>
				) : (
					<>
						{request.isError && (
							<Alert color="red" title="Proof not requested">
								{workspaceError(request.error)}
							</Alert>
						)}
						<Group gap="sm">
							<Button
								data-autofocus
								loading={request.isPending}
								onClick={() => request.mutate()}
							>
								Email me a proof
							</Button>
							<Button
								variant="default"
								disabled={request.isPending}
								onClick={onCancel}
							>
								Cancel
							</Button>
						</Group>
					</>
				)}
			</Stack>
		</Modal>
	);
}
