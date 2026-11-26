import { z } from "zod";
import { canManageTeam, type WorkspaceRole, workspaceRole } from "./workspace";

export type Member = { userId: string; role: WorkspaceRole; email?: string };

export function hasMultipleOwners(members: readonly Member[]): boolean {
	let found = false;
	for (const member of members) {
		if (member.role !== "owner") continue;
		if (found) return true;
		found = true;
	}
	return false;
}

export function memberAccess(
	actor: WorkspaceRole,
	target: WorkspaceRole,
	multipleOwners: boolean,
): "editable" | "last-owner" | "read-only" {
	if (!canManageTeam(actor)) return "read-only";
	if (target !== "owner") return "editable";
	if (actor !== "owner") return "read-only";
	return multipleOwners ? "editable" : "last-owner";
}

export function grantableRoles(
	actor: WorkspaceRole,
	current?: WorkspaceRole,
): WorkspaceRole[] {
	if (!canManageTeam(actor)) return [];
	return workspaceRole.options.filter(
		(role) => role !== current && (role !== "owner" || actor === "owner"),
	);
}

export type InvitationDraft = { email: string; role: WorkspaceRole };

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
	| { status: "invalid-email" | "invalid-role" }
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
	role: WorkspaceRole,
	actor: WorkspaceRole,
): InvitationReview {
	const recipient = invitationEmail.safeParse(email);
	if (!recipient.success) return { status: "invalid-email" };
	if (!grantableRoles(actor).includes(role)) return { status: "invalid-role" };
	return { status: "ready", draft: { email: recipient.data, role } };
}
