"""Check the CAD wiring against firmware without importing FreeCAD.

The v7 carrier's `RP_NETS` maps each RP2040-Zero pin (`IOn` is GPn) to the net the carrier wires
to it; `firmware/pip/src/profile.rs` declares the same pins. Both must agree.
"""

import ast
from pathlib import Path
import re
import unittest


class PinMapTest(unittest.TestCase):
    maxDiff = None

    def test_generator_nets_match_firmware(self):
        root = Path(__file__).resolve().parents[2]
        firmware = (root / "firmware/pip/src/profile.rs").read_text()
        tree = ast.parse((root / "hardware/enclosure-v7/build_case.py").read_text())
        assignments = {
            node.targets[0].id: node.value
            for node in tree.body
            if isinstance(node, ast.Assign)
            and len(node.targets) == 1
            and isinstance(node.targets[0], ast.Name)
        }

        def number(pattern, text=firmware):
            matches = re.findall(pattern, text)
            self.assertEqual(len(matches), 1, f"Expected one firmware declaration: {pattern}")
            return int(matches[0])

        def constant(name):
            return number(rf"pub const {name}: u8 = (\d+);")

        def array(name, size):
            found = re.findall(rf"pub const {name}: \[u8; {size}\] = (\[[\d, ]+\]);", firmware)
            self.assertEqual(len(found), 1, f"Expected one firmware declaration: {name}")
            pins = ast.literal_eval(found[0])
            self.assertEqual(len(pins), size)
            return pins

        rotary = re.findall(r"pub const ROTARY: RotaryProfile = RotaryProfile \{([^}]+)\};", firmware)
        self.assertEqual(len(rotary), 1)
        buttons = array("CONTEXT_BUTTONS", 3)
        clk = number(r"clk: (\d+),", rotary[0])
        dt = number(r"dt: (\d+),", rotary[0])
        sw = number(r"sw: (\d+),", rotary[0])
        expected = {
            f"IO{constant('SCK')}": "SCL",
            f"IO{constant('MOSI')}": "SDA",
            f"IO{constant('RST')}": "RES",
            f"IO{constant('DC')}": "DC",
            f"IO{constant('BL')}": "BLK",
            f"IO{clk}": "CLK",
            f"IO{dt}": "DT",
            f"IO{sw}": "SW",
            "3V3": "3V3",
            "GND": "GND",
            **{f"IO{pin}": f"B{i + 1}" for i, pin in enumerate(buttons)},
        }
        self.assertEqual(len(expected), 13, "two firmware pins share a number")

        nets = assignments["RP_NETS"]
        self.assertIsInstance(nets, ast.Dict)
        actual = {}
        for key, value in zip(nets.keys, nets.values):
            pin = ast.literal_eval(key)
            self.assertNotIn(pin, actual)
            actual[pin] = ast.literal_eval(value)
        self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
