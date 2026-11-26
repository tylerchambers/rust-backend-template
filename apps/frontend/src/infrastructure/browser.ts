export type WriteHeaders = { Origin: string; "x-csrf-token": string };

export function csrfToken(): string {
	return (
		document.cookie
			.split("; ")
			.find((part) => part.startsWith("csrf="))
			?.slice(5) ?? ""
	);
}

// The browser supplies Origin on the wire; the generated SDK also requires it in the DTO.
export function writeHeaders(): WriteHeaders {
	return { Origin: window.location.origin, "x-csrf-token": csrfToken() };
}
