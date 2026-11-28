#!/usr/bin/env python3
"""Log in to the demo in a real browser, so the tap page's script runs as it does for a person.

    python3 keycloak/demo/browser_smoke.py <keycloak base url> [<screenshot directory>]

Runs inside the image built from keycloak/demo/browser/Dockerfile; `make demo-smoke` starts it. Uses the realm's
`browser` user, so it does not collide with login_smoke.py. Three sign-ins, each in a fresh browser context:

1. Desktop: enrol by clicking on the image, with an undo along the way, then confirm.
2. Phone (emulated, touch): a wrong sequence is refused, then the right one, tapped, is accepted. Enrolled on a
   wide image, matched on a narrow one: taps are in units of image width (spec section 12).
3. Desktop, keyboard only: the arrow keys and Enter place the same sequence, and it is accepted.

Every submission is checked to carry a taps field of the same length, whatever the tap count. On failure, a
screenshot of the page is saved to the screenshot directory, if one is given.
"""

import re
import sys
import time
import urllib.parse
from pathlib import Path

from playwright.sync_api import Browser, Page, expect, sync_playwright

sys.path.insert(0, str(Path(__file__).resolve().parent))
from login_smoke import TAPS, WRONG, totp  # The same sequences and code maths.

USER = "browser"
OTP_SECRET = b"public-browser-otp-not-a-secret"  # Its own, so its codes never collide with login_smoke.py's.
PAIR = len("00.000000,00.000000")
STEP = 0.01  # One arrow key press, in units of image width, as pixel-factor-tap.js moves the cursor.


def fail(message):
    raise AssertionError(message)


def log_in(page: Page, base: str) -> int:
    """Log in as far as the tap page. Returns the time window whose code was used."""
    page.goto(f"{base}/realms/demo/account/")
    page.locator('input[name="username"]').fill(USER)
    page.locator('input[name="username"]').press("Enter")
    otp = page.locator('input[name="otp"]')
    expect(otp).to_be_visible()
    # Computed at the last moment: a code from a window that has just ended is refused.
    counter = int(time.time()) // 30
    otp.fill(totp(counter, OTP_SECRET))
    otp.press("Enter")
    expect(page.locator("#pf-stage")).to_be_visible()
    expect(page.locator("#pf-image")).to_have_js_property("complete", True)
    return counter


def place(page: Page, taps, touch: bool):
    image = page.locator("#pf-image")
    box = image.bounding_box()
    if not box or box["width"] <= 0:
        fail("tap page: image has no size")
    for x, y in taps:
        position = {"x": x * box["width"], "y": y * box["width"]}
        if touch:
            image.tap(position=position)
        else:
            image.click(position=position)
    expect(page.locator(".pf-dot")).to_have_count(len(taps))


def key_in(page: Page, taps):
    """Place taps with the keyboard alone. The cursor starts at the centre of the image."""
    stage = page.locator("#pf-stage")
    stage.focus()
    box = page.locator("#pf-image").bounding_box()
    if not box:
        fail("tap page: image has no size")
    cx, cy = 0.5, box["height"] / box["width"] / 2
    for x, y in taps:
        dx = round((x - cx) / STEP)
        dy = round((y - cy) / STEP)
        for _ in range(abs(dx)):
            stage.press("ArrowRight" if dx > 0 else "ArrowLeft")
        for _ in range(abs(dy)):
            stage.press("ArrowDown" if dy > 0 else "ArrowUp")
        stage.press("Enter")
        cx, cy = cx + dx * STEP, cy + dy * STEP
    expect(page.locator(".pf-dot")).to_have_count(len(taps))


def submit(page: Page):
    """Submit the tap form and check the field it sent has the fixed length."""
    slots = int(page.locator("#pf-stage").get_attribute("data-max-taps") or "0")
    with page.expect_request(lambda request: request.method == "POST") as posted:
        page.locator("#pf-submit").click()
    fields = urllib.parse.parse_qs(posted.value.post_data or "")
    sent = fields.get("taps", [""])[0]
    if len(sent) != slots * PAIR + slots - 1:
        fail(f"taps field is {len(sent)} characters for {slots} slots")
    page.wait_for_load_state()


def signed_in(page: Page, step: str):
    page.wait_for_url(re.compile(r"/realms/demo/account/"), timeout=30_000)
    expect(page.locator("#pf-stage")).to_have_count(0)
    print(f"ok  {step}")


def next_window(used: int):
    # Keycloak refuses a code it has already accepted, so each sign-in waits for a window not yet used.
    while int(time.time()) // 30 <= used:
        time.sleep(1)


def run(browser: Browser, base: str, phone: dict, shots: Path | None):
    steps = [
        ("desktop", {"viewport": {"width": 1280, "height": 900}}),
        ("phone", phone),
        ("keyboard", {"viewport": {"width": 1280, "height": 900}}),
    ]
    used = -1
    for name, options in steps:
        next_window(used)
        context = browser.new_context(**options)
        page = context.new_page()
        try:
            used = log_in(page, base)
            if name == "desktop":
                place(page, TAPS[:2] + [(0.5, 0.6)], touch=False)
                page.locator("#pf-undo").click()
                expect(page.locator(".pf-dot")).to_have_count(2)
                page.locator("#pf-clear").click()
                expect(page.locator(".pf-dot")).to_have_count(0)
                place(page, TAPS, touch=False)
                submit(page)
                expect(page.locator('button[name="restart"]')).to_be_visible()
                place(page, TAPS, touch=False)
                submit(page)
                signed_in(page, "desktop: enrolled by clicking, confirmed, signed in")
            elif name == "phone":
                place(page, WRONG, touch=True)
                submit(page)
                expect(page.get_by_text("That sequence did not match.")).to_be_visible()
                print("ok  phone: wrong sequence refused")
                place(page, TAPS, touch=True)
                submit(page)
                signed_in(page, "phone: right sequence tapped and accepted")
            else:
                key_in(page, TAPS)
                submit(page)
                signed_in(page, "keyboard: sequence placed with arrow keys and accepted")
        except Exception:
            if shots:
                shots.mkdir(parents=True, exist_ok=True)
                page.screenshot(path=str(shots / f"{name}.png"), full_page=True)
            raise
        finally:
            context.close()


def main():
    if len(sys.argv) not in (2, 3):
        print("usage: browser_smoke.py <keycloak base url> [<screenshot directory>]", file=sys.stderr)
        sys.exit(2)
    base = sys.argv[1].rstrip("/")
    shots = Path(sys.argv[2]) if len(sys.argv) == 3 else None
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        try:
            run(browser, base, playwright.devices["Pixel 7"], shots)
        except AssertionError as error:
            print(f"browser-smoke: {error}", file=sys.stderr)
            sys.exit(1)
        finally:
            browser.close()


if __name__ == "__main__":
    main()
