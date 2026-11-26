import {
	Alert,
	Anchor,
	Button,
	Paper,
	Stack,
	Text,
	Title,
} from "@mantine/core";
import { type JSX, useLayoutEffect, useRef, useState } from "react";
import { useAuthentication } from "../application/backend";
import { authenticationMessage } from "../infrastructure/authentication";

type ConfirmationState =
	| { status: "missing" | "ready" | "pending" }
	| { status: "error"; message: string };

export function ConfirmSignIn(): JSX.Element {
	const authentication = useAuthentication();
	const link = useRef<{ token: string } | null>(null);
	const [state, setState] = useState<ConfirmationState>({ status: "missing" });

	useLayoutEffect(() => {
		function capture(): void {
			const token = window.location.hash.slice(1);
			window.history.replaceState(null, "", window.location.pathname);
			link.current = token ? { token } : null;
			setState({ status: token ? "ready" : "missing" });
		}
		function onHashChange(event: HashChangeEvent): void {
			if (event.newURL === window.location.href) capture();
		}
		if (window.location.href.includes("#")) capture();
		window.addEventListener("hashchange", onHashChange);
		return () => window.removeEventListener("hashchange", onHashChange);
	}, []);

	async function confirm(): Promise<void> {
		const submitted = link.current;
		if (
			!submitted ||
			state.status === "pending" ||
			window.location.href.includes("#")
		)
			return;
		setState({ status: "pending" });
		let failure: string | undefined;
		try {
			await authentication.confirmLink(submitted.token);
		} catch (error) {
			failure = authenticationMessage(error);
		}
		// A replacement can reach the address bar before its hashchange event runs.
		if (submitted !== link.current || window.location.href.includes("#"))
			return;
		if (failure !== undefined) {
			setState({ status: "error", message: failure });
			return;
		}
		window.location.replace("/");
	}

	return (
		<Paper
			withBorder
			shadow="xs"
			radius="lg"
			maw={480}
			mx="auto"
			my="xl"
			p="xl"
		>
			<Stack gap="lg">
				<Stack gap={6}>
					<Text c="teal" size="xs" fw={700} tt="uppercase" lts="0.08em">
						Account access
					</Text>
					<Title order={1} size="h2">
						Confirm sign in
					</Title>
					{state.status === "missing" ? (
						<Text c="dimmed">
							Open the full link from your email, or return to Project desk to
							request a new one.
						</Text>
					) : (
						<Text c="dimmed">
							If you requested this sign-in link, confirm to open your workspace
							in this tab.
						</Text>
					)}
				</Stack>
				{state.status === "error" && (
					<Alert color="red" title="Sign-in not confirmed">
						{state.message}
					</Alert>
				)}
				<Button
					onClick={confirm}
					disabled={state.status === "missing"}
					loading={state.status === "pending"}
				>
					Confirm sign in
				</Button>
				<Anchor href="/" size="sm" ta="center">
					Back to Project desk
				</Anchor>
			</Stack>
		</Paper>
	);
}
