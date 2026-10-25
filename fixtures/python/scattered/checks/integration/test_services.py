from app.services import integration_only


def test_integration() -> None:
    assert integration_only() == "integration"
