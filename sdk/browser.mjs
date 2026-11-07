import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { chromium } from "playwright";

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  await page.goto("http://localhost:58039/v1/auth/confirmation");
  const email = `${randomUUID()}@example.test`;
  const requested = await page.evaluate(async (email) => {
    const csrf = document.cookie.split("; ").find((part) => part.startsWith("csrf="))?.slice(5);
    const response = await fetch("/v1/auth/request", {
      method: "POST",
      headers: { "content-type": "application/json", "x-csrf-token": csrf ?? "" },
      body: JSON.stringify({ email }),
    });
    return response.status;
  }, email);
  assert.equal(requested, 202);
  const listing = await (await page.request.get("http://127.0.0.1:58029/api/v1/messages")).json();
  const message = listing.messages.find((message) => message.To.some((to) => to.Address === email));
  assert.ok(message);
  const content = await (await page.request.get(`http://127.0.0.1:58029/api/v1/message/${message.ID}`)).json();
  const token = /confirmation#([a-f0-9]{64})/.exec(content.Text)?.[1];
  assert.ok(token);
  await page.goto(`http://localhost:58039/v1/auth/confirmation#${token}`);
  assert.equal((await page.request.get("http://localhost:58039/v1/auth/sessions")).status(), 401);
  const confirmationResponse = page.waitForResponse((response) => response.url().endsWith("/v1/auth/confirm"));
  await page.getByRole("button", { name: "Confirm sign in", exact: true }).click();
  const response = await confirmationResponse;
  assert.equal(response.status(), 200);
  await page.getByText("Signed in", { exact: true }).waitFor();
  assert.equal((await page.request.get("http://localhost:58039/v1/auth/sessions")).status(), 200);
  const cookies = await page.context().cookies();
  const session = cookies.find((cookie) => cookie.name === "__Host-session");
  assert.ok(session?.httpOnly && session.secure && session.sameSite === "Lax");
  const post = async (path, body, includeCsrf = true) => page.evaluate(async ({path, body, includeCsrf}) => {
    const csrf = document.cookie.split('; ').find(part => part.startsWith('csrf='))?.slice(5);
    const headers = {'content-type': 'application/json'};
    if (includeCsrf) headers['x-csrf-token'] = csrf ?? '';
    const response = await fetch(path, {method: 'POST', headers, body: JSON.stringify(body)});
    return {status: response.status, text: await response.text()};
  }, {path, body, includeCsrf});
  const organization = await post('/v1/organizations', {name: 'Browser proof tenant'});
  assert.equal(organization.status, 201);
  const org = JSON.parse(organization.text).id;
  const invitePath = `/v1/organizations/${org}/invitations`;
  const invitee = `${randomUUID()}@example.test`;
  assert.equal((await post(invitePath, {email: invitee, role: 'viewer', proof: ''})).status, 403);
  assert.equal((await post('/v1/auth/reauth/request', {action: 'invite', target: org, organization_id: org})).status, 202);
  const proofMessages = await (await page.request.get('http://127.0.0.1:58029/api/v1/messages')).json();
  const proofMessage = proofMessages.messages.find(message => message.Subject === 'Confirm privileged operation' && message.To.some(to => to.Address === email));
  assert.ok(proofMessage);
  const proofContent = await (await page.request.get(`http://127.0.0.1:58029/api/v1/message/${proofMessage.ID}`)).json();
  const proof = /confirmation#([a-f0-9]{64})/.exec(proofContent.Text)?.[1];
  assert.ok(proof);
  await page.goto(`http://localhost:58039/v1/auth/reauth/confirmation#${proof}`);
  assert.equal(await page.locator('#proof').textContent(), '');
  assert.equal((await post(invitePath, {email: invitee, role: 'viewer', proof}, false)).status, 403);
  await page.getByRole('button', {name: 'Confirm and reveal proof', exact: true}).click();
  assert.equal(await page.locator('#proof').textContent(), proof);
  assert.equal((await post(invitePath, {email: invitee, role: 'viewer', proof})).status, 201);
  assert.equal((await post(invitePath, {email: invitee, role: 'viewer', proof})).status, 403);
  console.log("PASS: native Chromium scanner GET, explicit confirmation POST, Secure HttpOnly session, action-bound email proof and CSRF/replay rejection");
} finally {
  await browser.close();
}
