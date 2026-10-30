"""Check the CAD wiring against firmware without importing FreeCAD."""

import ast
from pathlib import Path
import re
import unittest


class PinMapTest(unittest.TestCase):
    maxDiff = None

    def test_generator_nets_match_firmware(self):
        root = Path(__file__).resolve().parents[2]
        firmware = (root / "firmware/pip/src/profile.rs").read_text()
        tree = ast.parse((root / "hardware/enclosure/build_case.py").read_text())
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

        rotary = re.findall(r"pub const ROTARY: RotaryProfile = RotaryProfile \{([^}]+)\};", firmware)
        self.assertEqual(len(rotary), 1)
        buttons = re.findall(r"pub const CONTEXT_BUTTONS: \[u8; 3\] = (\[[\d, ]+\]);", firmware)
        self.assertEqual(len(buttons), 1)
        expected_buttons = ast.literal_eval(buttons[0])
        self.assertEqual(len(expected_buttons), 3)
        expected = {
            "SCL": f"IO{constant('SCK')}",
            "SDA": f"IO{constant('MOSI')}",
            "RES": f"IO{constant('RST')}",
            "DC": f"IO{constant('DC')}",
            "BLK": f"IO{constant('BL')}",
            "CLK": "IO" + str(number(r"clk: (\d+),", rotary[0])),
            "DT": "IO" + str(number(r"dt: (\d+),", rotary[0])),
            "SW": "IO" + str(number(r"sw: (\d+),", rotary[0])),
            "3V3": "3V3",
            "GND": "GND",
            **{f"B{i + 1}": f"IO{pin}" for i, pin in enumerate(expected_buttons)},
        }
        generator_buttons = ast.literal_eval(assignments["BTN_GPIO"])
        self.assertEqual(generator_buttons, [f"IO{pin}" for pin in expected_buttons])
        actual = {}
        nets = assignments["NETS"]
        self.assertIsInstance(nets, ast.Dict)
        for key, value in zip(nets.keys, nets.values):
            name = ast.literal_eval(key)
            self.assertNotIn(name, actual)
            self.assertIsInstance(value, ast.Tuple)
            pin = value.elts[0]
            if isinstance(pin, ast.Subscript):
                self.assertIsInstance(pin.value, ast.Name)
                self.assertEqual(pin.value.id, "BTN_GPIO")
                actual[name] = generator_buttons[ast.literal_eval(pin.slice)]
            else:
                actual[name] = ast.literal_eval(pin)
        self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
