#!/usr/bin/env python3
"""Compile one library test target once, then run only the selected libtest filters."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
PRESETS = {
    "revival": [
        "survivors::revival::", "survivors::enemies::pickup_tests::",
        "survivors::multiplayer::protocol::",
        "survivors::multiplayer::native::tests::fallen_guests_",
        "survivors::multiplayer::native::tests::a_living_guest_",
    ],
    "item-tiers": [
        "survivors::items::config::tests::level_bases_",
        "survivors::items::config::tests::generated_level_variants_",
        "survivors::items::config::tests::malformed_level_variants_",
        "survivors::items::visuals::", "survivors::items::item_level::",
        "survivors::items::tests::final_boss_chests_",
        "survivors::items::tests::ordinary_monster_chests_",
        "survivors::items::tests::boss_chests_",
    ],
    "ranged": [
        "survivors::ranged_enemies::", "survivors::expedition_config::",
        "survivors::training::", "survivors::multiplayer::native::tests::enemies_damage_the_nearest_guest_",
        "survivors::abilities::spell_engine_integration_tests::elemental_hits_",
    ],
    "difficulty": [
        "survivors::expedition_config::", "survivors::training::",
        "survivors::unit_frames::tests::",
        "survivors::multiplayer::native::tests::enemies_damage_the_nearest_guest_",
        "survivors::abilities::spell_engine_integration_tests::elemental_hits_",
    ],
    "levels": [
        "survivors::level_stats::", "survivors::talents::",
        "survivors::enemies::pickup_tests::", "survivors::spell_engine::tests::",
        "survivors::healer_buffs::", "survivors::combat_ui::sheet::tests::",
        "survivors::multiplayer::protocol::",
    ],
    "support": [
        "survivors::healer_buffs::",
        "survivors::abilities::spell_engine_integration_tests::healer_blessings_",
        "survivors::talent_runtime::",
        "survivors::combat_ui::sheet::tests::",
        "survivors::multiplayer::protocol::",
        "survivors::procs::",
        "survivors::items::armour::",
    ],
    "talents": [
        "survivors::minor_talents::",
        "survivors::arcane_repeats::",
        "survivors::talents::",
        "survivors::talent_runtime::",
        "survivors::starter_spells::",
        "survivors::procs::",
        "survivors::items::legendary::milestones::",
        "survivors::abilities::spell_engine_integration_tests::arcane_talents_",
        "survivors::spell_engine::tests::recasts",
        "survivors::spell_engine::tests::repeat_ordinals_",
        "survivors::spell_engine::tests::large_repeat_bursts_",
    ],
    "resistances": [
        "survivors::resistances::",
        "survivors::items::armour::",
        "survivors::items::config::tests::elemental_resistances_",
        "survivors::items::config::tests::all_resistance_",
        "survivors::items::config::tests::common_stats_and_resistances_",
        "survivors::abilities::spell_engine_integration_tests::elemental_hits_",
        "survivors::spell_engine::on_hit::",
        "survivors::expedition_config::",
    ],
    "combat": [
        "survivors::abilities::spell_engine_integration_tests::",
        "survivors::resistances::",
        "survivors::items::armour::",
        "survivors::spell_engine::on_hit::",
        "survivors::expedition_config::",
        "survivors::items::equipment::",
    ],
    "items": ["survivors::items::"],
    "expeditions": ["survivors::expedition_config::", "survivors::training::"],
}
CONFIG_FILTERS = {
    "config/survivors-expeditions.toml": PRESETS["expeditions"] + PRESETS["combat"],
    "config/survivors-item-affixes.toml": ["survivors::items::config::"],
    "config/survivors-item-bases.toml": [
        "survivors::items::config::", "survivors::items::equipment::",
        "survivors::items::visuals::", "survivors::items::weapons::",
        "survivors::items::armour::",
    ],
    "config/survivors-balance.toml": ["survivors::balance::", "survivors::spell_engine::"],
    "config/survivors-rewards.toml": ["survivors::reward_config::", "survivors::hud::passive_offer_tests::"],
}
BUILD_CONFIG = {"Cargo.toml", "Cargo.lock", ".cargo/config.toml", "rust-toolchain.toml"}


def compact(filters):
    """A module prefix subsumes all of its more specific filters."""
    result = []
    for item in sorted(set(filters), key=lambda f: (len(f), f)):
        if not any(item.startswith(parent) for parent in result):
            result.append(item)
    return sorted(result)


def path_filters(path, crate):
    path = path.replace("\\", "/")
    if path in CONFIG_FILTERS and crate == "benilla-app":
        return CONFIG_FILTERS[path]
    prefix = f"crates/{crate}/src/"
    if not path.startswith(prefix) or not path.endswith(".rs"):
        return []
    relative = path[len(prefix):-3]
    # Shared lifecycle/stat definitions can affect the entire Survivors simulation.
    if relative in ("survivors/mod", "survivors/server", "survivors/data"):
        return ["survivors::"]
    if relative == "survivors/healer_buffs":
        return PRESETS["support"]
    if relative == "survivors/level_stats":
        return PRESETS["levels"]
    if relative == "survivors/ranged_enemies":
        return PRESETS["ranged"]
    if relative == "survivors/revival":
        return PRESETS["revival"]
    if relative in ("survivors/talents", "survivors/minor_talents", "survivors/talent_design",
                    "survivors/talent_effects", "survivors/talent_runtime"):
        return PRESETS["talents"]
    if relative == "survivors/abilities":
        return ["survivors::abilities::spell_engine_integration_tests::"]
    if relative == "survivors/hero":
        return PRESETS["combat"]
    if relative in ("lib", "main"):
        return [""]
    parts = relative.split("/")
    if parts[-1] in ("mod", "tests"):
        parts.pop()
    return ["::".join(parts) + "::"] if parts else [""]


def changed_paths(base=None):
    git = ["git", "-c", f"safe.directory={ROOT.as_posix()}"]
    def read(*args):
        return subprocess.check_output(git + list(args), cwd=ROOT).decode().split("\0")
    paths = read("diff", "--name-only", "-z", base or "HEAD")
    paths += read("ls-files", "--others", "--exclude-standard", "-z")
    return sorted(set(filter(None, paths)))


def selected_tests(executable, filters, cwd):
    result = subprocess.run(
        [str(executable), "--list", "--format", "terse", *filters],
        cwd=cwd, text=True, capture_output=True, check=True,
    )
    return [line.removesuffix(": test") for line in result.stdout.splitlines() if line.endswith(": test")]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("filters", nargs="*", help="Libtest filters (OR, not shell expressions)")
    parser.add_argument("-p", "--package", default="benilla-app")
    parser.add_argument("--scope", choices=PRESETS)
    parser.add_argument("--path", action="append", default=[], help="Infer filters for this changed file; repeatable")
    parser.add_argument("--base", help="Compare against this Git revision instead of HEAD")
    parser.add_argument("--plan", action="store_true", help="Print selection without compiling")
    parser.add_argument("--list", action="store_true", help="Compile and list matching tests without running them")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--profile", default="test")
    args = parser.parse_args(argv)
    filters = list(args.filters)
    if args.scope:
        filters += PRESETS[args.scope]
    if args.path or not filters:
        paths = args.path or changed_paths(args.base)
        if not filters and any(path.replace("\\", "/") in BUILD_CONFIG for path in paths):
            parser.error("Build configuration changed. Choose a package and explicit filters or --scope; no automatic workspace build.")
        filters += [f for path in paths for f in path_filters(path, args.package)]
    filters = compact(filters)
    if not filters:
        parser.error("No library test scope inferred. Choose --scope, --path or explicit filters; use Cargo directly for integration tests.")
    print(f"Package: {args.package}; profile: {args.profile}", flush=True)
    print("Filters (OR): " + ", ".join(f or "all library tests" for f in filters), flush=True)
    command = ["cargo", "test", "-p", args.package, "--lib", "--profile", args.profile,
               "--no-run", "--message-format=json-render-diagnostics"]
    if args.offline:
        command.append("--offline")
    print("Build once: " + " ".join(command), flush=True)
    if args.plan:
        return 0

    started = time.monotonic()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--offline", "--format-version=1"], cwd=ROOT,
    ))
    package = next((p for p in metadata["packages"] if p["name"] == args.package), None)
    if package is None:
        parser.error(f"Unknown workspace package: {args.package}")
    cwd = Path(package["manifest_path"]).parent
    process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, text=True)
    executables = []
    for line in process.stdout:
        event = json.loads(line)
        if (event.get("reason") == "compiler-artifact" and event.get("executable")
                and event.get("profile", {}).get("test")
                and "lib" in event.get("target", {}).get("kind", [])):
            executables.append(Path(event["executable"]))
    if process.wait():
        return process.returncode
    if len(executables) != 1:
        parser.error("Expected exactly one library test executable from Cargo")
    executable = executables[0]
    tests = selected_tests(executable, filters, cwd)
    # A misspelled filter must not look like a successful test run.
    if not tests:
        parser.error("No tests match these filters. Choose the containing module or --scope.")
    unmatched = [f for f in filters if not any(f in name for name in tests)]
    if unmatched:
        parser.error("Filters matched no tests: " + ", ".join(unmatched))
    print(f"Selected {len(tests)} tests; build/check took {time.monotonic() - started:.2f}s", flush=True)
    if args.list:
        print("\n".join(tests))
        return 0
    return subprocess.run([str(executable), *filters], cwd=cwd).returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (subprocess.CalledProcessError, OSError, ValueError) as error:
        print(f"test-changed: {error}", file=sys.stderr)
        sys.exit(1)
