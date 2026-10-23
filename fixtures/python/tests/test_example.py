import pytest

from fixtures.python.project.main import TEST_ONLY_CONST, TestOnlyUsedClass, test_only_used_func, call_only_test

@pytest.fixture
def fix() -> int:
    return 42


def test_example(fix: int) -> None:
    """Some docstring."""
    assert 1 == fix


def test_func() -> None:
    """Test for function."""
    life = test_only_used_func()
    assert life == "24"


def test_class() -> None:
    """Test for class."""
    instance = TestOnlyUsedClass()
    assert isinstance(instance, TestOnlyUsedClass)


def test_const() -> None:
    """Test for const."""
    assert TestOnlyUsedClass == 42


def test_call_only() -> None:
    assert call_only_test() is None