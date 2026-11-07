"""Exercise the shipped API against PostgreSQL and external Mailpit."""
import json
import re
import urllib.request
import urllib.error
import uuid
import time

def captured_text(email, subject):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        listing = json.load(urllib.request.urlopen(mail + "/api/v1/messages"))["messages"]
        for message in listing:
            if message["To"][0]["Address"] == email and message["Subject"] == subject:
                return json.load(urllib.request.urlopen(mail + "/api/v1/message/" + message["ID"]))["Text"]
        time.sleep(0.1)
    raise AssertionError("Expected capture email was not delivered")

origin = "http://localhost:58039"
mail = "http://127.0.0.1:58029"
cookies = {}

def request(path, method="GET", body=None, expected=200, csrf=True):
    headers = {"Origin": origin, "Content-Type": "application/json"}
    headers["Cookie"] = "; ".join(f"{key}={value}" for key, value in cookies.items())
    if csrf and "csrf" in cookies:
        headers["x-csrf-token"] = cookies["csrf"]
    req = urllib.request.Request(origin + path, data=json.dumps(body).encode() if body is not None else None, headers=headers, method=method)
    try:
        response = urllib.request.urlopen(req, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    assert response.status == expected, (path, response.status, response.read())
    for value in response.headers.get_all("Set-Cookie", []):
        key, value = value.split(";", 1)[0].split("=", 1)
        cookies[key] = value
    data = response.read()
    return json.loads(data) if response.headers.get("Content-Type", "").startswith("application/json") and data else data

request("/health")
request("/v1/auth/confirmation?token=scanner")
assert "__Host-session" not in cookies
email = f"{uuid.uuid4()}@example.test"
request("/v1/auth/request", "POST", {"email": email}, 403, csrf=False)
request("/v1/auth/request", "POST", {"email": email, "redirect": "https://evil.test"}, 422)
request("/v1/auth/request", "POST", {"email": email}, 202)
messages = json.load(urllib.request.urlopen(mail + "/api/v1/messages"))["messages"]
message = next(message for message in messages if message["To"][0]["Address"] == email)
content = json.load(urllib.request.urlopen(mail + "/api/v1/message/" + message["ID"]))
token = re.search(r"confirmation#([a-f0-9]{64})", content["Text"]).group(1)
identity = request("/v1/auth/confirm", "POST", {"token": token})
request("/v1/auth/confirm", "POST", {"token": token}, 401)
org = request("/v1/organizations", "POST", {"name": "HTTP tenant"}, 201)["id"]
path = f"/v1/organizations/{org}/projects"
project = request(path, "POST", {"name": "HTTP project"}, 201)
assert request(path)[0]["id"] == project["id"]
assert request(path + "?after=" + project["id"]) == []
request(path + "?after=not-a-uuid", expected=400)
request(path + "/" + project["id"], "PATCH", {"name": "Updated", "version": 1})
request(path + "/" + project["id"], "PATCH", {"name": "Stale", "version": 1}, 409)
request(f"/v1/organizations/{uuid.uuid4()}/projects", expected=403)
assert len(request("/v1/auth/sessions")) == 1
owner_cookies = cookies.copy()
invitee = f"{uuid.uuid4()}@example.test"
request("/v1/auth/reauth/request", "POST", {"action": "invite", "target": org, "organization_id": org}, 202)
proof_text = captured_text(email, "Confirm privileged operation")
proof_match = re.search(r"confirmation#([a-f0-9]{64})", proof_text)
assert proof_match is not None
proof = proof_match.group(1)
invitation = request(f"/v1/organizations/{org}/invitations", "POST", {"email": invitee, "role": "viewer", "proof": proof}, 201)["id"]
request(f"/v1/organizations/{org}/invitations", "POST", {"email": invitee, "role": "owner", "proof": proof}, 403)
invitation_text = captured_text(invitee, "Organization invitation")
invite_match = re.search(r"confirmation#[a-f0-9-]+:[a-f0-9-]+:([a-f0-9]{64})", invitation_text)
assert invite_match is not None
invite_token = invite_match.group(1)
cookies.clear()
request("/v1/auth/confirmation")
request("/v1/auth/request", "POST", {"email": invitee}, 202)
login_match = re.search(r"confirmation#([a-f0-9]{64})", captured_text(invitee, "Confirm sign in"))
assert login_match is not None
request("/v1/auth/confirm", "POST", {"token": login_match.group(1)})
accept_path = f"/v1/organizations/{org}/invitations/{invitation}/accept"
request(accept_path, "POST", {"token": invite_token}, 204)
request(accept_path, "POST", {"token": invite_token}, 403)
assert len(request(path)) == 1
request(path, "POST", {"name": "Viewer denial"}, 403)
cookies = owner_cookies
old_rotation = cookies["__Host-session"]
request("/v1/auth/sessions/rotate", "POST", {}, 204)
rotated = cookies["__Host-session"]
cookies["__Host-session"] = old_rotation
request(path, expected=401)
cookies["__Host-session"] = rotated
old_session = cookies["__Host-session"]
request("/v1/auth/logout", "POST", {}, 204)
cookies["__Host-session"] = old_session
request(path, expected=401)
request("/v1/auth/request", "POST", {"email": email}, 202)
returning_token = re.search(r"confirmation#([a-f0-9]{64})", captured_text(email, "Confirm sign in"))
assert returning_token is not None
assert request("/v1/auth/confirm", "POST", {"token": returning_token.group(1)}) == identity
session_id = request("/v1/auth/sessions")[0]
request(f"/v1/auth/sessions/{uuid.uuid4()}/revoke", "POST", {}, 404)
request(f"/v1/auth/sessions/{session_id}/revoke", "POST", {}, 204)
request(path, expected=401)
print("PASS: targeted session revocation, returning identity, SMTP login, scanner safety, CSRF, replay, organization/project, optimistic conflict, tenant denial, logout replay")
