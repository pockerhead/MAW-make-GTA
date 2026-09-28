"""Offline gate for `package_release` exe checks: MSVC CRT import names and the PE subsystem (no console).

Run: python -m unittest tools/qa/test_package_release.py
"""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import package_release  # noqa: E402

CRT_NAMES = (
    "VCRUNTIME140.dll", "vcruntime140_1.dll", "vcruntime140_threads.dll",
    "MSVCP140.dll", "msvcp140_1.dll", "msvcp140_2.dll", "msvcp140_atomic_wait.dll", "msvcp140_codecvt_ids.dll",
    "ucrtbase.dll", "ucrtbased.dll", "api-ms-win-crt-runtime-l1-1-0.dll",
)
# A +crt-static exe still imports these (TASK-028 scratch/crt_static_probe).
NOT_CRT = ("api-ms-win-core-synch-l1-2-0.dll", "kernel32.dll", "ntdll.dll", "vcruntime140.pdb")


def pe(subsystem):
    """Minimal MZ/PE header bytes with IMAGE_OPTIONAL_HEADER.Subsystem = `subsystem`."""
    e_lfanew = 0x80
    data = bytearray(e_lfanew + 24 + 240)
    data[:2] = b"MZ"
    data[0x3C:0x40] = e_lfanew.to_bytes(4, "little")
    data[e_lfanew:e_lfanew + 4] = b"PE\0\0"
    data[e_lfanew + 24:e_lfanew + 26] = (0x20B).to_bytes(2, "little")
    data[e_lfanew + 24 + 68:e_lfanew + 24 + 70] = subsystem.to_bytes(2, "little")
    return bytes(data)


class CrtImports(unittest.TestCase):
    def test_every_crt_name_is_refused(self):
        for name in CRT_NAMES:
            with self.subTest(name=name):
                problems = package_release.dynamic_imports(b"\0" + name.encode() + b"\0", windows=True)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn(f"CRT dynamically: {name}", problems[0])

    def test_static_exe_names_pass(self):
        for name in NOT_CRT:
            with self.subTest(name=name):
                self.assertEqual(package_release.dynamic_imports(b"\0" + name.encode() + b"\0", windows=True), [])


class Subsystem(unittest.TestCase):
    def test_gui_exe_passes(self):
        self.assertEqual(package_release.pe_subsystem(pe(2)), 2)
        self.assertEqual(package_release.exe_problems(pe(2), windows=True), [])

    def test_console_exe_is_refused(self):
        problems = package_release.exe_problems(pe(3), windows=True)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("exe subsystem 3, expected 2 (GUI)", problems[0])

    def test_not_a_pe_is_refused(self):
        self.assertIsNone(package_release.pe_subsystem(b"\x7fELF" + bytes(300)))
        self.assertIn("exe subsystem None", package_release.exe_problems(b"MZ" + bytes(300), windows=True)[0])

    def test_linux_exe_has_no_subsystem_rule(self):
        self.assertEqual(package_release.exe_problems(b"\x7fELF" + bytes(300), windows=False), [])


if __name__ == "__main__":
    unittest.main()
