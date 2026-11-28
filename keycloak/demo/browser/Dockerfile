# The browser smoke test's image: Playwright's, which ships the browsers, plus the locked Python package. Built and
# run by scripts/demo-smoke.sh; never deployed. The image tag and the playwright pin in requirements.in must name the
# same release, or Playwright cannot find its browsers. Dependabot bumps each separately; merge them as one change.
#
# Regenerate requirements.txt with: uv pip compile --generate-hashes --universal --python-version 3.12 --no-header \
#   keycloak/demo/browser/requirements.in -o keycloak/demo/browser/requirements.txt
FROM mcr.microsoft.com/playwright/python:v1.65.2-noble@sha256:38dd67fc654b56b3cba611e4c8eb71cd33750e1cd29eea2a5dc00b484817a75f

COPY requirements.txt /tmp/requirements.txt
# The image has no venv support and this is a single-purpose test image, so install into the system Python.
RUN pip install --no-cache-dir --disable-pip-version-check --root-user-action=ignore --break-system-packages \
        --no-deps --require-hashes -r /tmp/requirements.txt \
    && rm /tmp/requirements.txt

USER pwuser
HEALTHCHECK NONE
ENTRYPOINT ["python3", "/demo/browser_smoke.py"]
