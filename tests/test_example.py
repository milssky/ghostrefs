import pytest


@pytest.fixture
def fix() -> int:
    return 42


def test_example(fix: int) -> None:
    """Some docstring."""
    assert 1 == fix