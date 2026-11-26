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

export function roleLabel(isAdmin: boolean): "Admin" | "Member" {
	return isAdmin ? "Admin" : "Member";
}
