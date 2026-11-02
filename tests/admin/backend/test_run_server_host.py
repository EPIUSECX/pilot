from __future__ import annotations

import pytest

from admin.backend.run_server import BIND_HOST_ENV, DEFAULT_BIND_HOST, resolve_bind_host


def test_default_listens_on_every_interface():
    assert resolve_bind_host({}) == DEFAULT_BIND_HOST
    assert resolve_bind_host({BIND_HOST_ENV: "  "}) == DEFAULT_BIND_HOST


@pytest.mark.parametrize("address", ["127.0.0.1", "::1"])
def test_loopback_address_is_honoured(address):
    assert resolve_bind_host({BIND_HOST_ENV: address}) == address


@pytest.mark.parametrize("value", ["localhost", "0.0.0.0/0", "not-an-address"])
def test_non_address_fails_loudly(value):
    with pytest.raises(SystemExit):
        resolve_bind_host({BIND_HOST_ENV: value})
