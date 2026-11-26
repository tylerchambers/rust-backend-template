import { z } from "zod";

export type Member = { userId: string; isAdmin: boolean; email?: string };

export function hasMultipleAdmins(members: readonly Member[]): boolean {
	let firstAdminId: string | undefined;
	for (const member of members) {
		if (!member.isAdmin) continue;
		if (firstAdminId !== undefined && firstAdminId !== member.userId)
			return true;
		firstAdminId = member.userId;
	}
	return false;
}

export type InvitationDraft = { email: string; isAdmin: boolean };

export const invitationStatus = z.enum([
	"pending",
	"accepted",
	"revoked",
	"expired",
]);

export function canRevokeInvitation(
	status: z.infer<typeof invitationStatus>,
): boolean {
	return status === "pending";
}

type InvitationReview =
	| { status: "invalid-email" }
	| { status: "ready"; draft: InvitationDraft };

const invitationEmail = z
	.string()
	.trim()
	.max(254)
	.pipe(z.email({ pattern: z.regexes.html5Email }))
	.refine((email) => {
		const local = email.slice(0, email.indexOf("@"));
		return (
			local.length <= 64 &&
			!local.startsWith(".") &&
			!local.endsWith(".") &&
			!local.includes("..")
		);
	}, "Use an unquoted email address without empty parts.")
	.transform((email) => email.toLowerCase());

export function reviewInvitation(
	email: string,
	isAdmin: boolean,
): InvitationReview {
	const recipient = invitationEmail.safeParse(email);
	if (!recipient.success) return { status: "invalid-email" };
	return { status: "ready", draft: { email: recipient.data, isAdmin } };
}
