import unittest

from stats import average, spread


class StatsTest(unittest.TestCase):
    def test_average(self):
        self.assertEqual(average([2, 4, 6]), 4)

    def test_average_of_nothing_is_zero(self):
        self.assertEqual(average([]), 0.0)

    def test_spread(self):
        self.assertEqual(spread([3, 9, 5]), 6)


if __name__ == "__main__":
    unittest.main()
