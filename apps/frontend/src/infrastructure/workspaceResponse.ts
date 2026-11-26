import { z } from "zod";

class WorkspaceHttpError extends Error {
	constructor(readonly status: number) {
		super(`Workspace request failed (${status})`);
	}
}

export function workspaceBody(result: {
	response: Response;
	data?: unknown;
}): unknown {
	if (!result.response.ok) throw new WorkspaceHttpError(result.response.status);
	return result.data;
}

export function isVersionConflict(error: unknown): boolean {
	return error instanceof WorkspaceHttpError && error.status === 409;
}

export function isWorkspaceUnavailableToUser(error: unknown): boolean {
	return (
		error instanceof WorkspaceHttpError &&
		(error.status === 401 || error.status === 403 || error.status === 404)
	);
}

export function workspaceError(error: unknown): string {
	if (error instanceof WorkspaceHttpError) {
		switch (error.status) {
			case 400:
			case 422:
				return "The server could not accept this request. Check the details and try again.";
			case 401:
				return "Your session has expired. Sign in again to continue.";
			case 403:
				return "You don’t have permission to do that in this organization.";
			case 404:
				return "This item no longer exists or is not available to you.";
			case 409:
				return "This request conflicts with the current state. Refresh and review the details.";
			case 429:
				return "Too many requests. Wait a moment before trying again.";
		}
	}
	if (error instanceof z.ZodError)
		return "The server returned unexpected workspace data.";
	return "The request could not be completed. Check your connection and try again.";
}
