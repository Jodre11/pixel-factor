#!/usr/bin/env python3
"""Log in to the demo through Keycloak's real pages, as a browser would, without JavaScript.

    python3 keycloak/demo/login_smoke.py <keycloak base url>

Enrols the demo user's taps (first entry, then confirmation) after the one-time code, then logs in again: a wrong
sequence must be refused and the right one accepted. Builds the taps field exactly as pixel-factor-tap.js does.
Standard library only. Exits non-zero at the first step that does not answer as expected.
"""

import base64
import hashlib
import hmac
import html
import http.cookiejar
import re
import secrets
import struct
import sys
import time
import urllib.parse
import urllib.request
from typing import NoReturn

OTP_SECRET = b"public-demo-otp-not-a-secret"
TAPS = [(0.10, 0.10), (0.30, 0.20), (0.50, 0.30), (0.70, 0.40), (0.90, 0.50)]
WRONG = [(0.10, 0.10), (0.30, 0.20), (0.50, 0.30), (0.70, 0.40), (0.90, 0.60)]
FILLER = "99.999999,99.999999"


def fail(message) -> NoReturn:
    print(f"login-smoke: {message}", file=sys.stderr)
    sys.exit(1)


def totp(counter, secret=OTP_SECRET):
    """RFC 6238 code, SHA-1, 6 digits, 30 s, as the demo realm's OTP policy."""
    digest = hmac.new(secret, struct.pack(">Q", counter), hashlib.sha1).digest()
    offset = digest[-1] & 0x0F
    code = (struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7FFFFFFF) % 1_000_000
    return f"{code:06d}"


def field(taps, max_taps):
    pairs = [f"{x:09.6f},{y:09.6f}" for x, y in taps]
    pairs += [FILLER] * (max_taps - len(pairs))
    return ";".join(pairs)


class LocalPolicy(http.cookiejar.DefaultCookiePolicy):
    """Send Secure cookies over plain HTTP, as browsers do for a loopback address. The demo is never on HTTPS."""

    def return_ok_secure(self, cookie, request):
        return True


class Browser:
    def __init__(self, base):
        self.base = base.rstrip("/")
        jar = http.cookiejar.CookieJar(LocalPolicy())
        self.opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))

    def get(self, url):
        with self.opener.open(url, timeout=30) as response:
            return response.geturl(), response.read().decode()

    def post(self, url, data):
        body = urllib.parse.urlencode(data).encode()
        with self.opener.open(urllib.request.Request(url, data=body), timeout=60) as response:
            return response.geturl(), response.read().decode()

    def start_login(self, extra=None):
        verifier = secrets.token_urlsafe(48)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).decode().rstrip("=")
        query = urllib.parse.urlencode({
            "client_id": "account-console",
            "redirect_uri": f"{self.base}/realms/demo/account/",
            "response_type": "code",
            "scope": "openid",
            "code_challenge": challenge,
            "code_challenge_method": "S256",
            **(extra or {}),
        })
        return self.get(f"{self.base}/realms/demo/protocol/openid-connect/auth?{query}")


def action(page):
    match = re.search(r'<form[^>]*action="([^"]+)"', page)
    if not match:
        fail("no form on the page")
    return html.unescape(match.group(1))


def expect(page, fragment, step):
    if fragment not in page:
        excerpt = re.sub(r"\s+", " ", page)[:600]
        fail(f"{step}: page lacks {fragment!r}: {excerpt}")
    print(f"ok  {step}")


def through_possession(browser):
    """Log in as far as the tap step. Returns the page and the time window whose code was used."""
    _, page = browser.start_login()
    expect(page, 'name="username"', "username page")
    _, page = browser.post(action(page), {"username": "demo"})
    expect(page, 'name="otp"', "one-time code page")
    # Computed at the last moment: a code from a window that has just ended is refused.
    counter = int(time.time()) // 30
    _, page = browser.post(action(page), {"otp": totp(counter)})
    return page, counter


def max_taps(page):
    match = re.search(r'data-max-taps="(\d+)"', page)
    if not match:
        fail("tap page has no data-max-taps")
    return int(match.group(1))


def main():
    if len(sys.argv) != 2:
        fail("usage: login_smoke.py <keycloak base url>")
    base = sys.argv[1]

    browser = Browser(base)
    page, used = through_possession(browser)
    expect(page, 'id="pf-stage"', "enrolment tap page after the code")
    image = re.search(r'src="([^"]+/img/demo\.svg)"', page)
    if not image:
        fail("tap page does not show the demo image")
    _, svg = browser.get(urllib.parse.urljoin(base, html.unescape(image.group(1))))
    expect(svg, "<svg", "demo image served from the jar")
    slots = max_taps(page)
    _, page = browser.post(action(page), {"taps": field(TAPS, slots)})
    expect(page, 'name="restart"', "confirmation page after staging")
    url, page = browser.post(action(page), {"taps": field(TAPS, slots)})
    if "code=" not in url:
        fail(f"enrolment did not complete the login: ended at {url}")
    print("ok  enrolment confirmed and login completed")

    # A fresh browser: no cookie, so the whole flow runs again. Keycloak refuses a code from a window it has already
    # accepted one from, so wait for the next window.
    while int(time.time()) // 30 <= used:
        time.sleep(1)
    browser = Browser(base)
    page, _ = through_possession(browser)
    expect(page, 'id="pf-stage"', "login tap page after the code")
    slots = max_taps(page)
    _, page = browser.post(action(page), {"taps": field(WRONG, slots)})
    expect(page, "That sequence did not match.", "wrong sequence refused")
    url, page = browser.post(action(page), {"taps": field(TAPS, slots)})
    if "code=" not in url:
        fail(f"the right sequence did not complete the login: ended at {url}")
    print("ok  right sequence accepted")

    # Re-enrolment from a signed-in browser, as the account console starts it. The session cookie alone must not
    # reach the tap page: the action asks for a fresh login, so the possession factor runs again first.
    _, page = browser.start_login({"kc_action": "pixel-factor-enrol"})
    if 'id="pf-stage"' in page:
        fail("re-enrolment reached the tap page on the session cookie alone")
    print("ok  re-enrolment asks for a fresh login first")


if __name__ == "__main__":
    main()
