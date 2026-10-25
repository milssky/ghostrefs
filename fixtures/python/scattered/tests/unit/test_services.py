from app.services import unit_only


def test_unit() -> None:
    assert unit_only() == "unit"
