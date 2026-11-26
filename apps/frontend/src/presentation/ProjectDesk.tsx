import { Anchor, Box, Container, Group } from "@mantine/core";
import type { JSX, ReactNode } from "react";

export function ProjectDesk({
	homeHref,
	children,
}: {
	homeHref: string;
	children: ReactNode;
}): JSX.Element {
	return (
		<>
			<Box component="header" className="masthead">
				<Container size="md">
					<Group justify="space-between" py="md">
						<a href={homeHref} className="wordmark">
							<span className="brand-mark" aria-hidden="true">
								P
							</span>
							Project desk
						</a>
						<Anchor href="/openapi.json" c="gray.2" size="sm">
							API contract
						</Anchor>
					</Group>
				</Container>
			</Box>
			<Container component="main" size="md" py="xl">
				{children}
			</Container>
		</>
	);
}
