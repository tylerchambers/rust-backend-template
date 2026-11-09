"""Pinned fixture images and canonical development port settings."""

POSTGRES = "docker.io/library/postgres:17-alpine@sha256:18cfe3ef5e6815560c98237d6216d1e5119702fb0f3894c8785dd58b8bbe5d73"
PLAYWRIGHT = "mcr.microsoft.com/playwright:v1.61.1-noble@sha256:5b8f294aff9041b7191c34a4bab3ac270157a28774d4b0660e9743297b697e48"


def dependency_fixtures(tls):
    return [
        (
            "postgres",
            POSTGRES,
            [
                "-p",
                "127.0.0.1:55439:5432",
                "-e",
                "POSTGRES_PASSWORD=local-template-test",
                "-e",
                "POSTGRES_DB=template",
            ],
        ),
        (
            "mail",
            "docker.io/axllent/mailpit:v1.27.8@sha256:6abc8e633df15eaf785cfcf38bae48e66f64beecdc03121e249d0f9ec15f0707",
            [
                "-p",
                "127.0.0.1:58029:8025",
                "-p",
                "127.0.0.1:51029:1025",
                "-e",
                "MP_DISABLE_VERSION_CHECK=true",
                "-v",
                f"{tls}:/tls:ro",
                "-e",
                "MP_SMTP_TLS_CERT=/tls/cert.pem",
                "-e",
                "MP_SMTP_TLS_KEY=/tls/key.pem",
                "-e",
                "MP_SMTP_AUTH_ACCEPT_ANY=true",
            ],
        ),
        (
            "redis",
            "docker.io/library/redis:7.4.2-alpine@sha256:02419de7eddf55aa5bcf49efb74e88fa8d931b4d77c07eff8a6b2144472b6952",
            ["-p", "127.0.0.1:56379:6379"],
        ),
    ]
