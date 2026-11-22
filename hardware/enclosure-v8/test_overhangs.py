"""Geometry regressions; run in freecad.cmd as well as the ordinary Python suite.

The system Python skips CAD tests when FreeCAD's snap module is unavailable.
"""
from pathlib import Path
import runpy
import unittest

try:
    import FreeCAD as App
    import Part
except ImportError:
    App = Part = None


@unittest.skipUnless(App is not None, "run CAD geometry tests with freecad.cmd")
class OverhangTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cad = runpy.run_path(str(Path(__file__).with_name("build_case.py")), run_name="overhang_tests")

    def check(self, shape, name="Probe"):
        printed = self.cad["PRINTED"]
        printed.clear()
        printed[name] = shape
        return self.cad["check_overhangs"]()

    def test_small_face_cantilever_is_not_skipped(self):
        # 0.02 mm2 underside: the old 0.05 mm2 face filter silently ignored it.
        support = Part.makeBox(0.2, 0.02, 2)
        roof = Part.makeBox(1.2, 0.02, 0.2, App.Vector(0, 0, 2))
        self.assertTrue(self.check(support.fuse(roof).removeSplitter()))

    def test_bridge_over_ten_mm_fails(self):
        supports = Part.makeBox(1, 3, 2).fuse(Part.makeBox(1, 3, 2, App.Vector(12, 0, 0)))
        roof = Part.makeBox(13, 3, 1, App.Vector(0, 0, 2))
        self.assertTrue(self.check(supports.fuse(roof).removeSplitter()))

    def test_short_bridge_passes(self):
        supports = Part.makeBox(1, 3, 2).fuse(Part.makeBox(1, 3, 2, App.Vector(5, 0, 0)))
        roof = Part.makeBox(6, 3, 1, App.Vector(0, 0, 2))
        self.assertEqual(self.check(supports.fuse(roof).removeSplitter()), [])

    def test_steep_cantilever_fails(self):
        v = App.Vector
        profile = [v(0, 0, 0), v(1, 0, 0), v(3, 0, 1.4), v(3, 0, 2), v(0, 0, 2)]
        shape = Part.Face(Part.makePolygon(profile + profile[:1])).extrude(v(0, 2, 0))
        self.assertTrue(self.check(shape))

    def test_carrier_prints_flat(self):
        cad = self.cad
        plate = Part.makeBox(12, 12, cad["C_T"], App.Vector(0, 0, cad["C_BACK"]))
        posed = cad["print_pose"]("Carrier", plate)
        self.assertAlmostEqual(posed.BoundBox.ZMin, 0.0)
        self.assertAlmostEqual(posed.BoundBox.ZLength, 2.0)
        self.assertEqual(self.check(plate, "Carrier"), [])

    def test_knob_flush_engagement_and_tip_clearance(self):
        cad = self.cad
        knob = cad["make_knob"]()
        cad["PRINTED"].clear()
        cad["PRINTED"]["Knob"] = knob
        cad["DUMMIES"]["Dummy_EncoderNut"] = cad["dummy_encoder_nut"]()
        engagement, rest, pressed, nut = cad["check_knob_engagement"]()
        self.assertAlmostEqual(engagement, 3.0)
        self.assertAlmostEqual(knob.BoundBox.ZMin, 4.3)
        cx, cy = cad["KNOB_C"]
        tip = cad["HW040_FRONT"] + cad["EC11_SHAFT_H"]
        probe = Part.makeLine(App.Vector(cx, cy, tip), App.Vector(cx, cy, 16))
        ceiling = min(e.BoundBox.ZMin for e in probe.common(knob).Edges)
        self.assertAlmostEqual(ceiling - tip, 1.0)
        self.assertGreaterEqual(min(pressed, nut), 0.5)
