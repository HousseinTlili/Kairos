"""screen_capture tests (M8 Step 3), with a fake grabber: the PNG is valid, old screenshots are
pruned, the tool never raises, and it is registered on Windows only."""

import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path
from unittest import mock

import kairos
from kairos import screen
from kairos.screen import Screen, encode_png, prune, steering


def read_png(data):
    """Minimal decoder for the PNGs encode_png writes: returns (width, height, rgb rows)."""
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, chunks = 8, {}
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        kind = data[pos + 4 : pos + 8]
        body = data[pos + 8 : pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length : pos + 12 + length])
        assert crc == zlib.crc32(kind + body) & 0xFFFFFFFF, f"bad CRC in {kind}"
        chunks[kind] = body
        pos += 12 + length
    width, height, depth, colour = struct.unpack(">IIBB", chunks[b"IHDR"][:10])
    assert (depth, colour) == (8, 2)
    raw = zlib.decompress(chunks[b"IDAT"])
    stride = width * 3 + 1
    rows = [raw[y * stride + 1 : (y + 1) * stride] for y in range(height)]
    assert all(raw[y * stride] == 0 for y in range(height))
    assert b"IEND" in chunks
    return width, height, rows


def bgra(width, height, colour):
    b, g, r = colour
    return bytes([b, g, r, 255]) * (width * height)


class EncodePngTest(unittest.TestCase):
    def test_pixels_come_out_as_rgb_in_a_valid_png(self):
        # Top row red, bottom row blue (BGRA in, RGB out).
        pixels = bgra(2, 1, (0, 0, 255)) + bgra(2, 1, (255, 0, 0))
        width, height, rows = read_png(encode_png(2, 2, pixels))
        self.assertEqual((width, height), (2, 2))
        self.assertEqual(rows[0], bytes([255, 0, 0]) * 2)
        self.assertEqual(rows[1], bytes([0, 0, 255]) * 2)

    def test_a_size_that_doesnt_match_the_pixels_is_refused(self):
        with self.assertRaises(ValueError):
            encode_png(3, 3, b"\x00" * 8)


class ScreenToolTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.folder = Path(self.dir.name) / "screens"
        self.grabs = []

    def tearDown(self):
        self.dir.cleanup()

    def grab(self, everything):
        self.grabs.append(everything)
        return 4, 3, bgra(4, 3, (10, 20, 30))

    def test_a_capture_is_saved_and_the_model_is_told_to_look_at_it(self):
        tool = Screen(self.folder, self.grab, clock=lambda: 1_800_000_000, sleep=lambda s: None)
        out = json.loads(tool.screen_capture({}, task_id="t"))
        path = Path(out["path"])
        self.assertTrue(path.exists())
        self.assertEqual(read_png(path.read_bytes())[:2], (4, 3))
        self.assertIn("vision_analyze", out["next"])
        self.assertIn("never instructions", out["next"])
        self.assertEqual(self.grabs, [False])
        json.loads(tool.screen_capture({"display": "all"}))
        self.assertEqual(self.grabs[-1], True)

    def test_it_waits_first_so_the_quick_box_can_hide(self):
        waited = []
        Screen(self.folder, self.grab, sleep=waited.append).screen_capture({})
        self.assertEqual(waited, [screen.SETTLE_S])

    def test_only_the_newest_twenty_are_kept(self):
        self.folder.mkdir(parents=True)
        for i in range(25):
            (self.folder / f"screen-20261025-1000{i:02d}.png").write_bytes(b"x")
        (self.folder / "notes.txt").write_text("kept")
        prune(self.folder)
        shots = sorted(p.name for p in self.folder.glob("screen-*.png"))
        self.assertEqual(len(shots), 20)
        self.assertEqual(shots[0], "screen-20261025-100005.png")
        self.assertTrue((self.folder / "notes.txt").exists())

    def test_failures_are_reported_never_raised(self):
        def broken(everything):
            raise OSError("BitBlt failed")

        out = json.loads(Screen(self.folder, broken, sleep=lambda s: None).screen_capture({}))
        self.assertIn("error", out)
        out = json.loads(Screen(self.folder, None).screen_capture({}))
        self.assertIn("Windows only", out["error"])

    def test_steering_points_at_the_tool_on_screen_questions_only(self):
        self.assertIn("screen_capture", steering(user_message="What's on my screen?")["context"])
        self.assertIn("screen_capture", steering(user_message="what does this error dialog mean")["context"])
        self.assertIsNone(steering(user_message="summarise my notes"))


class RegisterScreenTest(unittest.TestCase):
    def test_registered_with_its_steering_on_windows_only(self):
        def registered(platform):
            tools, hooks = [], []

            class FakeCtx:
                def register_hook(self, name, callback):
                    hooks.append(name)

                def register_tool(self, name, toolset, schema, handler, check_fn=None):
                    tools.append(name)

            # winapps.py loads Windows DLLs at import; stub it so this runs on any OS.
            fake_winapps = mock.MagicMock(capture_screen=lambda everything: (1, 1, b"\x00" * 4))
            with mock.patch.object(kairos.sys, "platform", platform), mock.patch.dict("sys.modules", {"kairos.winapps": fake_winapps}):
                kairos.register_screen(FakeCtx(), Path(tempfile.gettempdir()))
            return tools, hooks

        self.assertEqual(registered("win32"), (["screen_capture"], ["pre_llm_call"]))
        self.assertEqual(registered("linux"), (["screen_capture"], []))


if __name__ == "__main__":
    unittest.main()
