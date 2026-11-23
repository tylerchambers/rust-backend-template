"""Exercise the shipped API against PostgreSQL and external Mailpit."""

import json
import re
import time
import urllib.error
import urllib.request
import uuid
from datetime import datetime, timedelta


def captured_text(email, subject):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        listing = json.load(urllib.request.urlopen(mail + "/api/v1/messages"))[
            "messages"
        ]
        for message in listing:
            if message["To"][0]["Address"] == email and message["Subject"] == subject:
                return json.load(
                    urllib.request.urlopen(mail + "/api/v1/message/" + message["ID"])
                )["Text"]
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
    req = urllib.request.Request(
        origin + path,
        data=json.dumps(body).encode() if body is not None else None,
        headers=headers,
        method=method,
    )
    try:
        response = urllib.request.urlopen(req, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    assert response.status == expected, (path, response.status, response.read())
    if path == "/v1/me" and response.status == 200:
        assert response.headers["Cache-Control"] == "no-store"
    for value in response.headers.get_all("Set-Cookie", []):
        key, value = value.split(";", 1)[0].split("=", 1)
        cookies[key] = value
    data = response.read()
    return (
        json.loads(data)
        if response.headers.get("Content-Type", "").startswith("application/json")
        and data
        else data
    )


request("/health")
assert request("/v1/me", expected=401, csrf=False) == {"code": "unauthenticated"}
request("/v1/auth/confirmation?token=scanner")
assert "__Host-session" not in cookies
email = f"{uuid.uuid4()}@example.test"
request("/v1/auth/request", "POST", {"email": email}, 403, csrf=False)
request(
    "/v1/auth/request", "POST", {"email": email, "redirect": "https://evil.test"}, 422
)
request("/v1/auth/request", "POST", {"email": email}, 202)
messages = json.load(urllib.request.urlopen(mail + "/api/v1/messages"))["messages"]
message = next(message for message in messages if message["To"][0]["Address"] == email)
content = json.load(urllib.request.urlopen(mail + "/api/v1/message/" + message["ID"]))
token = re.search(r"confirmation#([a-f0-9]{64})", content["Text"]).group(1)
identity = request("/v1/auth/confirm", "POST", {"token": token})
owner_account = {"user_id": identity["user_id"], "email": email}
assert request("/v1/me", csrf=False) == owner_account
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
request(
    "/v1/auth/reauth/request",
    "POST",
    {"action": "invite", "target": org, "organization_id": org},
    202,
)
proof_text = captured_text(email, "Confirm privileged operation")
proof_match = re.search(r"confirmation#([a-f0-9]{64})", proof_text)
assert proof_match is not None
proof = proof_match.group(1)
invitation = request(
    f"/v1/organizations/{org}/invitations",
    "POST",
    {"email": invitee, "role": "viewer", "proof": proof},
    201,
)["id"]
invitations_path = f"/v1/organizations/{org}/invitations"
(pending,) = request(invitations_path, csrf=False)
assert set(pending) == {"id", "email", "role", "status", "expires_at"}
assert pending["id"] == invitation
assert pending["email"] == invitee
assert pending["role"] == "viewer"
assert pending["status"] == "pending"
assert datetime.fromisoformat(pending["expires_at"]).utcoffset() == timedelta(0)
request(
    f"/v1/organizations/{org}/invitations",
    "POST",
    {"email": invitee, "role": "owner", "proof": proof},
    403,
)
invitation_text = captured_text(invitee, "Organization invitation")
invite_match = re.search(
    r"confirmation#[a-f0-9-]+:[a-f0-9-]+:([a-f0-9]{64})", invitation_text
)
assert invite_match is not None
invite_token = invite_match.group(1)
other_org = request("/v1/organizations", "POST", {"name": "Other HTTP tenant"}, 201)[
    "id"
]
other_projects_path = f"/v1/organizations/{other_org}/projects"
foreign = request(other_projects_path, "POST", {"name": "Foreign"}, 201)
cookies.clear()
request("/v1/auth/confirmation")
request("/v1/auth/request", "POST", {"email": invitee}, 202)
login_match = re.search(
    r"confirmation#([a-f0-9]{64})", captured_text(invitee, "Confirm sign in")
)
assert login_match is not None
invitee_identity = request("/v1/auth/confirm", "POST", {"token": login_match.group(1)})
invitee_account = {"user_id": invitee_identity["user_id"], "email": invitee}
assert request("/v1/me", csrf=False) == invitee_account
accept_path = f"/v1/organizations/{org}/invitations/{invitation}/accept"
request(accept_path, "POST", {"token": invite_token}, 204)
request(accept_path, "POST", {"token": invite_token}, 403)
assert len(request(path)) == 1
assert request(path + "/" + project["id"], csrf=False) == {
    "id": project["id"],
    "organization_id": org,
    "name": "Updated",
    "version": 2,
    "archived": False,
}
for missing_id in (str(uuid.uuid4()), foreign["id"]):
    assert request(path + "/" + missing_id, expected=404, csrf=False) == {
        "code": "not_found"
    }
assert request(other_projects_path + "/" + foreign["id"], expected=403, csrf=False) == {
    "code": "forbidden"
}
assert request(path + "/invalid", expected=400, csrf=False) == {"code": "invalid_input"}
request(path, "POST", {"name": "Viewer denial"}, 403)
invitee_cookies = cookies.copy()
assert request("/v1/organizations") == [
    {"id": org, "name": "HTTP tenant", "role": "viewer"}
]
members_path = f"/v1/organizations/{org}/members"
members = sorted(
    [
        {"user_id": identity["user_id"], "role": "owner"},
        {"user_id": invitee_identity["user_id"], "role": "viewer"},
    ],
    key=lambda member: member["user_id"],
)
assert request(members_path, csrf=False) == members
audit_path = f"/v1/organizations/{org}/audit-events"
directory_path = f"/v1/organizations/{org}/member-directory"
for collection in (invitations_path, audit_path, directory_path):
    assert request(collection, expected=403, csrf=False) == {"code": "forbidden"}
request(f"/v1/organizations/{uuid.uuid4()}/members", expected=403)
cookies = owner_cookies
assert request(members_path, csrf=False) == members
assert request(invitations_path, csrf=False) == [{**pending, "status": "accepted"}]
directory = sorted(
    [{**owner_account, "role": "owner"}, {**invitee_account, "role": "viewer"}],
    key=lambda member: member["user_id"],
)
assert request(directory_path, csrf=False) == directory
events = request(audit_path, csrf=False)
for event in events:
    assert set(event) == {"id", "actor_id", "action", "subject", "occurred_at"}
    assert datetime.fromisoformat(event["occurred_at"]).utcoffset() == timedelta(0)
assert any(
    event["actor_id"] == identity["user_id"]
    and event["action"] == "project.updated"
    and event["subject"] == project["id"]
    for event in events
)
for collection in (invitations_path, audit_path, directory_path):
    for query in ("?after=invalid", "?offset=1"):
        assert request(collection + query, expected=400, csrf=False) == {
            "code": "invalid_input"
        }
# Exercise actual bounded HTTP traversal, not only a storage cursor.
expected_ids = {project["id"]}
for index in range(50):
    expected_ids.add(request(path, "POST", {"name": f"Page {index}"}, 201)["id"])
seen = []
cursor = ""
while True:
    page = request(path + cursor)
    assert len(page) <= 50
    if not page:
        break
    seen.extend(row["id"] for row in page)
    cursor = "?after=" + page[-1]["id"]
assert len(seen) == len(set(seen)) == 51 and set(seen) == expected_ids
# Foreign-tenant cursors are ordering values, never tenant authority.
assert all(
    row["organization_id"] == org for row in request(path + "?after=" + foreign["id"])
)
assert len(request("/v1/organizations")) == 2
assert len(request("/v1/organizations?after=" + org)) == 1
request("/v1/organizations?after=invalid", expected=400)
request(f"/v1/organizations/{org}/members?after=invalid", expected=400)
request(
    "/v1/auth/reauth/request",
    "POST",
    {
        "action": "change_role",
        "target": invitee_identity["user_id"],
        "organization_id": org,
    },
    202,
)
role_match = re.search(
    r"confirmation#([a-f0-9]{64})", captured_text(email, "Confirm privileged operation")
)
assert role_match is not None
role_proof = role_match.group(1)
request(
    f"/v1/organizations/{org}/members/{invitee_identity['user_id']}/role",
    "POST",
    {"role": "member", "proof": role_proof},
    204,
)
assert request(directory_path, csrf=False) == sorted(
    [{**owner_account, "role": "owner"}, {**invitee_account, "role": "member"}],
    key=lambda member: member["user_id"],
)
cookies = invitee_cookies
assert request("/v1/me", expected=401, csrf=False) == {"code": "unauthenticated"}
request(path, expected=401)
cookies = owner_cookies
assert len(request(path)) == 50
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
returning_token = re.search(
    r"confirmation#([a-f0-9]{64})", captured_text(email, "Confirm sign in")
)
assert returning_token is not None
assert (
    request("/v1/auth/confirm", "POST", {"token": returning_token.group(1)}) == identity
)
assert request("/v1/me", csrf=False) == owner_account
session_id = request("/v1/auth/sessions")[0]
request(f"/v1/auth/sessions/{uuid.uuid4()}/revoke", "POST", {}, 404)
request(f"/v1/auth/sessions/{session_id}/revoke", "POST", {}, 204)
request(path, expected=401)
print(
    "PASS: targeted session revocation, returning identity, SMTP login, scanner safety, CSRF, replay, organization/project, optimistic conflict, tenant denial, logout replay"
)
