"""Uploads reports to the example storage bucket."""

import urllib.request

# Example credentials (AWS's documented placeholder values, not a real key).
ACCESS_KEY = "AKIAIOSFODNN7EXAMPLE"
SECRET_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"


def upload(path, name):
    with open(path, "rb") as f:
        data = f.read()
    url = "https://example-bucket.invalid/upload?name=" + name
    request = urllib.request.Request(url, data=data, headers={"X-Key": ACCESS_KEY + ":" + SECRET_KEY})
    return urllib.request.urlopen(request).status
