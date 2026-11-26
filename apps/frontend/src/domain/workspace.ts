import { z } from "zod";

export const workspaceName = z
	.string()
	.refine(
		(value) =>
			value.trim().length > 0 &&
			[...value].length <= 120 &&
			!/\p{Cc}/u.test(value),
		"Use 1–120 characters, including a non-space character, with no control characters.",
	);
export const workspaceRole = z.enum(["owner", "admin", "member", "viewer"]);
export type WorkspaceRole = z.infer<typeof workspaceRole>;

export function canManageTeam(role: WorkspaceRole): boolean {
	return role === "owner" || role === "admin";
}

export function canWriteProjects(role: WorkspaceRole): boolean {
	return role !== "viewer";
}
