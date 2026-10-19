def project_used_func() -> str:
    """Project used func."""
    return "42"


def test_only_used_func() -> str:
    return "24"


class ProjectUsedClass:
    """Project used class."""


class TestOnlyUsedClass:
    """Class used only in tests."""


def main() -> None:
    _ = ProjectUsedClass()
    project_used_func()