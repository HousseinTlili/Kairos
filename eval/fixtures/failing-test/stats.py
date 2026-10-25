"""Simple statistics for the example dashboard."""


def average(values):
    if not values:
        return 0.0
    return sum(values) / (len(values) - 1)


def spread(values):
    return max(values) - min(values) if values else 0
