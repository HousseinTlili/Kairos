"""The Windows calls behind the PC control tools (M3 §6), through `ctypes` so the plugin stays
standard library only. Imported only on Windows; `pctools.py` holds the logic and its tests.
"""

from __future__ import annotations

import ctypes
import json
import os
import subprocess
from ctypes import wintypes

from .pctools import Window

CREATE_NO_WINDOW = 0x0800_0000
START_APPS_TIMEOUT = 15

WM_CLOSE = 0x0010
GW_OWNER = 4
GWL_EXSTYLE = -20
WS_EX_TOOLWINDOW = 0x0000_0080
DWMWA_CLOAKED = 14
PROCESS_TERMINATE = 0x0001
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
SYNCHRONIZE = 0x0010_0000
WAIT_TIMEOUT = 0x0000_0102

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
dwmapi = ctypes.WinDLL("dwmapi")

WNDENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

user32.EnumWindows.argtypes = [WNDENUMPROC, wintypes.LPARAM]
user32.EnumWindows.restype = wintypes.BOOL
user32.IsWindowVisible.argtypes = [wintypes.HWND]
user32.IsWindowVisible.restype = wintypes.BOOL
user32.IsWindow.argtypes = [wintypes.HWND]
user32.IsWindow.restype = wintypes.BOOL
user32.GetWindow.argtypes = [wintypes.HWND, wintypes.UINT]
user32.GetWindow.restype = wintypes.HWND
user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
user32.GetWindowLongW.restype = wintypes.LONG
user32.GetWindowTextLengthW.argtypes = [wintypes.HWND]
user32.GetWindowTextLengthW.restype = ctypes.c_int
user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.GetWindowTextW.restype = ctypes.c_int
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowThreadProcessId.restype = wintypes.DWORD
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.PostMessageW.restype = wintypes.BOOL
kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel32.OpenProcess.restype = wintypes.HANDLE
kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
kernel32.CloseHandle.restype = wintypes.BOOL
kernel32.QueryFullProcessImageNameW.argtypes = [
    wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)
]
kernel32.QueryFullProcessImageNameW.restype = wintypes.BOOL
kernel32.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
kernel32.TerminateProcess.restype = wintypes.BOOL
kernel32.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
kernel32.WaitForSingleObject.restype = wintypes.DWORD
dwmapi.DwmGetWindowAttribute.argtypes = [wintypes.HWND, wintypes.DWORD, ctypes.c_void_p, wintypes.DWORD]
dwmapi.DwmGetWindowAttribute.restype = ctypes.c_long

# Prints UTF-8 JSON whatever the console code page, so app names with accents survive.
_START_APPS_SCRIPT = (
    "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; "
    "Get-StartApps | Select-Object Name, AppID | ConvertTo-Json -Compress"
)


def _exe_name(pid: int) -> str:
    handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return ""
    try:
        size = wintypes.DWORD(1024)
        buffer = ctypes.create_unicode_buffer(size.value)
        if not kernel32.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size)):
            return ""
        return os.path.basename(buffer.value)
    finally:
        kernel32.CloseHandle(handle)


def _cloaked(hwnd: int) -> bool:
    """Suspended Store apps keep 'visible' windows that aren't on screen; DWM reports them cloaked."""
    value = wintypes.DWORD(0)
    result = dwmapi.DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, ctypes.byref(value), ctypes.sizeof(value))
    return result == 0 and value.value != 0


class Win32Layer:
    def start_apps(self) -> list[tuple[str, str]]:
        done = subprocess.run(
            ["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", _START_APPS_SCRIPT],
            capture_output=True,
            timeout=START_APPS_TIMEOUT,
            creationflags=CREATE_NO_WINDOW,
            check=True,
        )
        data = json.loads(done.stdout.decode("utf-8-sig") or "[]")
        if isinstance(data, dict):  # ConvertTo-Json gives an object, not a list, for one app
            data = [data]
        return [
            (str(item.get("Name") or ""), str(item.get("AppID") or ""))
            for item in data
            if isinstance(item, dict)
        ]

    def launch(self, target: str) -> None:
        os.startfile(target)  # noqa: S606 — targets come from the catalogue, never from the model

    def windows(self) -> list[Window]:
        found: list[Window] = []

        def visit(hwnd, _lparam):
            if not user32.IsWindowVisible(hwnd) or user32.GetWindow(hwnd, GW_OWNER):
                return True
            if user32.GetWindowLongW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW or _cloaked(hwnd):
                return True
            length = user32.GetWindowTextLengthW(hwnd)
            if length <= 0:
                return True
            buffer = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(hwnd, buffer, length + 1)
            pid = wintypes.DWORD(0)
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            found.append(Window(hwnd=int(hwnd), pid=int(pid.value), exe=_exe_name(pid.value), title=buffer.value))
            return True

        user32.EnumWindows(WNDENUMPROC(visit), 0)
        return found

    def post_close(self, hwnd: int) -> None:
        user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)

    def window_open(self, hwnd: int) -> bool:
        return bool(user32.IsWindow(hwnd) and user32.IsWindowVisible(hwnd))

    def terminate(self, pid: int) -> None:
        handle = kernel32.OpenProcess(PROCESS_TERMINATE, False, pid)
        if handle:
            try:
                kernel32.TerminateProcess(handle, 1)
            finally:
                kernel32.CloseHandle(handle)

    def process_running(self, pid: int) -> bool:
        handle = kernel32.OpenProcess(SYNCHRONIZE, False, pid)
        if not handle:
            return False
        try:
            return kernel32.WaitForSingleObject(handle, 0) == WAIT_TIMEOUT
        finally:
            kernel32.CloseHandle(handle)


# --- Screen capture (M8 §4) ----------------------------------------------------------------------

gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

SM_CXSCREEN, SM_CYSCREEN = 0, 1
SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN = 76, 77, 78, 79
SRCCOPY = 0x00CC0020
CAPTUREBLT = 0x40000000
DIB_RGB_COLORS = 0
# Per-monitor-aware for this thread only, so the copy is in real pixels without changing how the
# Hermes process itself is scaled.
DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = ctypes.c_void_p(-4)


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wintypes.DWORD),
        ("biWidth", wintypes.LONG),
        ("biHeight", wintypes.LONG),
        ("biPlanes", wintypes.WORD),
        ("biBitCount", wintypes.WORD),
        ("biCompression", wintypes.DWORD),
        ("biSizeImage", wintypes.DWORD),
        ("biXPelsPerMeter", wintypes.LONG),
        ("biYPelsPerMeter", wintypes.LONG),
        ("biClrUsed", wintypes.DWORD),
        ("biClrImportant", wintypes.DWORD),
    ]


user32.GetSystemMetrics.argtypes = [ctypes.c_int]
user32.GetSystemMetrics.restype = ctypes.c_int
user32.GetDC.argtypes = [wintypes.HWND]
user32.GetDC.restype = wintypes.HDC
user32.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
user32.ReleaseDC.restype = ctypes.c_int
gdi32.CreateCompatibleDC.argtypes = [wintypes.HDC]
gdi32.CreateCompatibleDC.restype = wintypes.HDC
gdi32.CreateCompatibleBitmap.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
gdi32.CreateCompatibleBitmap.restype = wintypes.HBITMAP
gdi32.SelectObject.argtypes = [wintypes.HDC, wintypes.HGDIOBJ]
gdi32.SelectObject.restype = wintypes.HGDIOBJ
gdi32.BitBlt.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.DWORD]
gdi32.BitBlt.restype = wintypes.BOOL
gdi32.GetDIBits.argtypes = [wintypes.HDC, wintypes.HBITMAP, wintypes.UINT, wintypes.UINT, ctypes.c_void_p, ctypes.c_void_p, wintypes.UINT]
gdi32.GetDIBits.restype = ctypes.c_int
gdi32.DeleteObject.argtypes = [wintypes.HGDIOBJ]
gdi32.DeleteObject.restype = wintypes.BOOL
gdi32.DeleteDC.argtypes = [wintypes.HDC]
gdi32.DeleteDC.restype = wintypes.BOOL


def capture_screen(everything: bool) -> tuple[int, int, bytes]:
    """(width, height, BGRA top-down) of the primary display, or of all displays."""
    previous = None
    set_context = getattr(user32, "SetThreadDpiAwarenessContext", None)
    if set_context is not None:
        set_context.argtypes = [ctypes.c_void_p]
        set_context.restype = ctypes.c_void_p
        previous = set_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
    try:
        if everything:
            x, y = user32.GetSystemMetrics(SM_XVIRTUALSCREEN), user32.GetSystemMetrics(SM_YVIRTUALSCREEN)
            width, height = user32.GetSystemMetrics(SM_CXVIRTUALSCREEN), user32.GetSystemMetrics(SM_CYVIRTUALSCREEN)
        else:
            x, y = 0, 0
            width, height = user32.GetSystemMetrics(SM_CXSCREEN), user32.GetSystemMetrics(SM_CYSCREEN)
        screen = user32.GetDC(None)
        memory = gdi32.CreateCompatibleDC(screen)
        bitmap = gdi32.CreateCompatibleBitmap(screen, width, height)
        old = gdi32.SelectObject(memory, bitmap)
        try:
            if not gdi32.BitBlt(memory, 0, 0, width, height, screen, x, y, SRCCOPY | CAPTUREBLT):
                raise OSError("BitBlt failed")
            header = BITMAPINFOHEADER()
            header.biSize = ctypes.sizeof(BITMAPINFOHEADER)
            header.biWidth = width
            header.biHeight = -height  # negative: top row first
            header.biPlanes = 1
            header.biBitCount = 32
            buffer = ctypes.create_string_buffer(width * height * 4)
            if gdi32.GetDIBits(memory, bitmap, 0, height, buffer, ctypes.byref(header), DIB_RGB_COLORS) != height:
                raise OSError("GetDIBits failed")
            return width, height, buffer.raw
        finally:
            gdi32.SelectObject(memory, old)
            gdi32.DeleteObject(bitmap)
            gdi32.DeleteDC(memory)
            user32.ReleaseDC(None, screen)
    finally:
        if set_context is not None and previous:
            set_context(previous)
