from app.services import colocated_only


def test_colocated() -> None:
    assert colocated_only() == "colocated"
