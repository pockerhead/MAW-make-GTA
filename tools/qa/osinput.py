"""QA helper: real OS mouse/keyboard input into the game window (Windows SendInput via ctypes).
Used instead of BRP holds, which never release while Time<Virtual> is paused."""
import ctypes
import time
from ctypes import wintypes

user32 = ctypes.WinDLL("user32", use_last_error=True)
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)
except Exception:
    user32.SetProcessDPIAware()

MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP = 0x0002, 0x0004
KEYEVENTF_KEYUP = 0x0002
VK = {"Escape": 0x1B, "Enter": 0x0D, "Alt": 0x12}


def window(title, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        hwnd = user32.FindWindowW(None, title)
        if hwnd:
            return hwnd
        time.sleep(0.2)
    raise RuntimeError(f"window {title!r} not found")


def focus(hwnd):
    # An Alt tap on a window that already has focus opens Win32 menu mode, which eats the next click.
    if user32.GetForegroundWindow() == hwnd:
        return True
    # Windows refuses SetForegroundWindow from a background process unless an Alt tap came first.
    user32.keybd_event(VK["Alt"], 0, 0, 0)
    user32.keybd_event(VK["Alt"], 0, KEYEVENTF_KEYUP, 0)
    user32.ShowWindow(hwnd, 9)
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.3)
    return user32.GetForegroundWindow() == hwnd


def click(hwnd, x, y):
    """Left click at client-area physical pixel (x, y)."""
    pt = wintypes.POINT(int(round(x)), int(round(y)))
    user32.ClientToScreen(hwnd, ctypes.byref(pt))
    user32.SetCursorPos(pt.x, pt.y)
    time.sleep(0.15)
    user32.mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0)
    time.sleep(0.08)
    user32.mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0)
    time.sleep(0.3)


def key(name):
    vk = VK[name]
    user32.keybd_event(vk, 0, 0, 0)
    time.sleep(0.08)
    user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, 0)
    time.sleep(0.3)
