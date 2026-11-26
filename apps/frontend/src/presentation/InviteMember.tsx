import {
	Button,
	Group,
	NativeSelect,
	Stack,
	TextInput,
	Title,
} from "@mantine/core";
import { Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { useInvitationManagement } from "../application/teamManagement";
import { type InvitationDraft, reviewInvitation } from "../domain/team";
import { roleLabel } from "../domain/workspace";
import { ConfirmAction } from "./ConfirmAction";

export function InviteMember({
	organizationId,
}: {
	organizationId: string;
}): React.JSX.Element {
	const navigate = useNavigate();
	const { invite } = useInvitationManagement(organizationId);
	const [email, setEmail] = useState("");
	const [isAdmin, setIsAdmin] = useState(false);
	const [draft, setDraft] = useState<InvitationDraft | null>(null);
	const [sent, setSent] = useState(false);
	const review = reviewInvitation(email, isAdmin);
	useEffect(() => {
		if (sent) {
			void navigate({
				to: "/organizations/$organizationId/settings",
				params: { organizationId },
				replace: true,
			});
		}
	}, [sent, navigate, organizationId]);

	return (
		<>
			<form
				className="entry-form"
				aria-label="Invite member"
				onSubmit={(event) => {
					event.preventDefault();
					if (!draft && !sent && review.status === "ready")
						setDraft(review.draft);
				}}
			>
				<Stack gap="sm">
					<Title order={1}>Invite member</Title>
					<TextInput
						label="Recipient email"
						type="email"
						required
						maxLength={254}
						autoCapitalize="none"
						spellCheck={false}
						value={email}
						error={
							email && review.status === "invalid-email"
								? "Enter a valid email address."
								: undefined
						}
						onChange={(event) => setEmail(event.currentTarget.value)}
					/>
					<NativeSelect
						label="Invitation role"
						value={isAdmin ? "admin" : "member"}
						onChange={(event) =>
							setIsAdmin(event.currentTarget.value === "admin")
						}
					>
						<option value="member">Member</option>
						<option value="admin">Admin</option>
					</NativeSelect>
					<Group>
						<Button type="submit" disabled={sent || review.status !== "ready"}>
							Review invitation
						</Button>
						<Link
							to="/organizations/$organizationId/settings"
							params={{ organizationId }}
						>
							Cancel
						</Link>
					</Group>
				</Stack>
			</form>
			{draft && !sent && (
				<ConfirmAction
					label="Send invitation"
					description={`Invite ${draft.email} as ${roleLabel(draft.isAdmin)}?`}
					submit={() => invite(draft)}
					onCancel={() => setDraft(null)}
					onSuccess={() => setSent(true)}
				/>
			)}
		</>
	);
}
