"""Exercise the shipped API against PostgreSQL and external Mailpit."""
import json
import re
import urllib.request
import urllib.error
import uuid

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
request(path + "/" + project["id"], "PATCH", {"name": "Updated", "version": 1})
request(path + "/" + project["id"], "PATCH", {"name": "Stale", "version": 1}, 409)
request(f"/v1/organizations/{uuid.uuid4()}/projects", expected=403)
old_session = cookies["__Host-session"]
request("/v1/auth/logout", "POST", {}, 204)
cookies["__Host-session"] = old_session
request(path, expected=401)
print("PASS: SMTP login, scanner safety, CSRF, replay, organization/project, optimistic conflict, tenant denial, logout replay")
