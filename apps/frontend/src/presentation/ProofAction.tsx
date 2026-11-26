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
import { useMutation } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { useBackend } from "../bootstrap/context";
import { writeHeaders } from "../infrastructure/browser";
import { workspaceBody, workspaceError } from "../infrastructure/workspace";

type Props = {
	organizationId: string;
	action: "invite" | "revoke_invitation" | "change_role" | "remove_member";
	target: string;
	label: string;
	description: string;
	submit: (proof: string) => Promise<void>;
	onSuccess: () => void;
	onCancel: () => void;
};

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
	const [submitting, setSubmitting] = useState(false);
	const [error, setError] = useState("");
	const request = useMutation({
		networkMode: "always",
		retry: false,
		mutationFn: async () => {
			workspaceBody(
				await client.POST("/v1/auth/reauth/request", {
					params: { header: writeHeaders() },
					body: { organization_id: organizationId, action, target },
					parseAs: "text",
					redirect: "error",
				}),
			);
		},
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
	return (
		<Paper withBorder p="md" component="section">
			<Stack gap="sm">
				<Title order={3}>{label}</Title>
				<Text>{description}</Text>
				<Text size="sm" c="dimmed" style={{ overflowWrap: "anywhere" }}>
					Organization: {organizationId}
					{target !== organizationId && <> · Target: {target}</>}
				</Text>
				{request.isSuccess ? (
					<form onSubmit={confirm}>
						<Stack gap="sm">
							<Text size="sm">
								Open the confirmation email in this browser. Check the action
								and target, choose “Confirm and reveal proof”, then paste the
								proof here. It expires after five minutes.
							</Text>
							<TextInput
								name="proof"
								label="Email proof"
								type="password"
								autoComplete="off"
								spellCheck={false}
								required
								disabled={submitting}
							/>
							{error && (
								<Alert color="red" title="Action not confirmed">
									{error} If the proof expired or was already used, request a
									new one.
								</Alert>
							)}
							<Group>
								<Button type="submit" loading={submitting}>
									{label}
								</Button>
								<Button
									type="button"
									variant="subtle"
									disabled={submitting}
									onClick={() => {
										request.reset();
										setError("");
									}}
								>
									Use a new proof
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
						<Button
							style={{ alignSelf: "start" }}
							loading={request.isPending}
							onClick={() => request.mutate()}
						>
							Email me a proof
						</Button>
					</>
				)}
				<Button
					variant="default"
					style={{ alignSelf: "start" }}
					disabled={submitting || request.isPending}
					onClick={onCancel}
				>
					Cancel
				</Button>
			</Stack>
		</Paper>
	);
}
