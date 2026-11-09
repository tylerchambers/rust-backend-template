"""Parse both live scrapes with the official Python OpenMetrics parser."""

import urllib.request

from prometheus_client.openmetrics.parser import text_string_to_metric_families

for path in ["/metrics", "/metrics/http"]:
    with urllib.request.urlopen(
        "http://localhost:58039" + path, timeout=10
    ) as response:
        assert response.headers["Content-Type"] == (
            "application/openmetrics-text; version=1.0.0; charset=utf-8"
        )
        text = response.read().decode()
    families = {family.name: family for family in text_string_to_metric_families(text)}
    histogram = families["http_request_duration_seconds"]
    assert histogram.type == "histogram"
    samples = {sample.name: sample.value for sample in histogram.samples}
    assert samples["http_request_duration_seconds_count"] > 0
    assert samples["http_request_duration_seconds_sum"] > 0
    buckets = [s for s in histogram.samples if s.name.endswith("_bucket")]
    assert len(buckets) > 2
    assert buckets[-1].labels == {"le": "+Inf"}
    assert buckets[-1].value == samples["http_request_duration_seconds_count"]
    assert all(a.value <= b.value for a, b in zip(buckets, buckets[1:], strict=False))
    # There are intentionally no per-user, path, mailbox, tenant or token labels.
    assert all(set(s.labels) <= {"le"} for f in families.values() for s in f.samples)
    assert families["http_requests"].type == "counter"
    if path == "/metrics":
        assert families["river_attempt_retries"].type == "gauge"
        assert families["db_pool_connections"].type == "gauge"
print(
    "PASS: both live OpenMetrics scrapes parse, histogram is cumulative, labels bounded"
)
