import pytest

from static.python.project.main import TestOnlyUsedClass, test_only_used_func 

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