"""Production functions consumed from scattered test locations."""


def unit_only() -> str:
    return "unit"


def integration_only() -> str:
    return "integration"


def colocated_only() -> str:
    return "colocated"


def production_used() -> str:
    return "production"


def main() -> None:
    print(production_used())
