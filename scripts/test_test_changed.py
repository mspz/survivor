"""Selection and fail-closed behavior, without compiling the game."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("test_changed", Path(__file__).with_name("test-changed.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class SelectionTests(unittest.TestCase):
    def test_common_definitions_keep_the_full_simulation_in_scope(self):
        self.assertEqual(runner.path_filters("crates/benilla-app/src/survivors/server.rs", "benilla-app"), ["survivors::"])
        self.assertEqual(runner.compact(["survivors::", "survivors::items::", "survivors::resistances::"]), ["survivors::"])

    def test_configuration_selects_its_consumers(self):
        self.assertIn("survivors::abilities::spell_engine_integration_tests::", runner.path_filters("config/survivors-expeditions.toml", "benilla-app"))
        self.assertEqual(runner.path_filters("config/survivors-item-affixes.toml", "benilla-app"), ["survivors::items::config::"])
        self.assertIn("survivors::minor_talents::", runner.path_filters("crates/benilla-app/src/survivors/talent_design.rs", "benilla-app"))
        self.assertIn("survivors::abilities::spell_engine_integration_tests::arcane_talents_", runner.path_filters("crates/benilla-app/src/survivors/talent_effects.rs", "benilla-app"))

    def test_module_paths_accept_windows_separators(self):
        self.assertEqual(runner.path_filters(r"crates\benilla-app\src\survivors\items\armour.rs", "benilla-app"), ["survivors::items::armour::"])
        self.assertEqual(runner.path_filters("crates/benilla-app/src/survivors/spell_engine/mod.rs", "benilla-app"), ["survivors::spell_engine::"])
        self.assertEqual(runner.path_filters("crates/benilla-app/src/survivors/spell_engine/tests.rs", "benilla-app"), ["survivors::spell_engine::"])

    def test_unrelated_paths_do_not_claim_coverage(self):
        for path in ["docs/METHOD.md", "scripts/other.py", "Cargo.toml", "crates/benilla-ui/src/script.rs"]:
            self.assertEqual(runner.path_filters(path, "benilla-app"), [])

    def test_multiple_filters_are_passed_together_and_only_tests_are_counted(self):
        with patch.object(runner.subprocess, "run", return_value=SimpleNamespace(stdout="one: test\ntwo: benchmark\nthree: test\n")) as run:
            selected = runner.selected_tests(Path("unit-tests.exe"), ["one", "three"], Path("."))
        self.assertEqual(selected, ["one", "three"])
        self.assertEqual(run.call_args.args[0][-2:], ["one", "three"])

    def test_plan_never_starts_cargo(self):
        with patch.object(runner.subprocess, "Popen") as build, patch.object(runner.subprocess, "check_output") as metadata:
            self.assertEqual(runner.main(["--scope", "resistances", "--plan"]), 0)
        build.assert_not_called()
        metadata.assert_not_called()

    def test_no_inferred_scope_is_an_error(self):
        with self.assertRaises(SystemExit) as error:
            runner.main(["--path", "Cargo.toml", "--plan"])
        self.assertEqual(error.exception.code, 2)

    def test_build_config_changes_require_an_explicit_scope_even_with_module_changes(self):
        with self.assertRaises(SystemExit) as error:
            runner.main(["--path", "Cargo.toml", "--path", "crates/benilla-app/src/survivors/resistances.rs", "--plan"])
        self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
