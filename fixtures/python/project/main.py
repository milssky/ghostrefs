from typing import Final

CODE_ONLY_CONST: Final = 42
TEST_ONLY_CONST: Final = 43

def project_used_func() -> str:
    """Project used func."""
    return "42"


def test_only_used_func() -> str:
    return "24"
    

def test_only_used_in_other_func() -> str:
    return "24"


def call_only_test() -> str:
    test_only_used_in_other_func()


class ProjectUsedClass:
    """Project used class."""


class TestOnlyUsedClass:
    """Class used only in tests."""


def main() -> None:
    _ = ProjectUsedClass()
    project_used_func()
    print(CODE_ONLY_CONST)