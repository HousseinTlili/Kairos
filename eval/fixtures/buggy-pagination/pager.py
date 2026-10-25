"""Splits a list into pages for the web UI."""


def page(items, number, size=10):
    """Return page `number` (1-based) of `items`, `size` items per page."""
    if number < 1 or size < 1:
        raise ValueError("number and size must be positive")
    start = (number - 1) * size
    end = start + size
    return items[start:end - 1]


def page_count(items, size=10):
    return (len(items) + size - 1) // size
