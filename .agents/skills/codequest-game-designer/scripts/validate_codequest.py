#!/usr/bin/env python3
"""Fast authoring validation for CODEQUEST.toml schema v1.

The Rust engine parser remains authoritative. This standard-library validator
gives the design skill a quick check without building the desktop application.
"""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path
from typing import Any


TOP_LEVEL_KEYS = {"schema_version", "game", "scenes", "mechanics", "art"}
GAME_KEYS = {"type", "title", "summary", "start_scene"}
SCENE_KEYS = {"id", "title", "kind", "summary", "mechanics", "art", "next"}
MECHANIC_KEYS = {"id", "summary", "inputs", "rules", "feedback"}
ART_KEYS = {"id", "kind", "summary", "requirements"}


class ContractError(ValueError):
    pass


def require_keys(table: dict[str, Any], allowed: set[str], location: str) -> None:
    unknown = sorted(set(table) - allowed)
    if unknown:
        raise ContractError(f"{location} has unknown field `{unknown[0]}`")


def require_text(value: Any, location: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ContractError(f"{location} must be a non-empty string")
    return value


def optional_text(table: dict[str, Any], key: str, location: str) -> None:
    if key in table:
        require_text(table[key], f"{location}.{key}")


def string_list(table: dict[str, Any], key: str, location: str) -> list[str]:
    value = table.get(key, [])
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise ContractError(f"{location}.{key} must be an array of strings")
    return value


def table_list(config: dict[str, Any], key: str) -> list[dict[str, Any]]:
    value = config.get(key, [])
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        raise ContractError(f"{key} must be an array of tables")
    return value


def collect_ids(items: list[dict[str, Any]], kind: str) -> set[str]:
    ids: set[str] = set()
    for index, item in enumerate(items):
        item_id = require_text(item.get("id"), f"{kind}[{index}].id")
        if item_id in ids:
            raise ContractError(f"duplicate {kind} id `{item_id}`")
        ids.add(item_id)
    return ids


def require_reference(reference: str, known: set[str], location: str) -> None:
    if reference not in known:
        raise ContractError(f"{location} references missing id `{reference}`")


def validate(config: dict[str, Any]) -> tuple[int, int, int]:
    require_keys(config, TOP_LEVEL_KEYS, "root")
    schema_version = config.get("schema_version")
    if type(schema_version) is not int or schema_version != 1:
        raise ContractError("schema_version must equal 1")

    game = config.get("game")
    if not isinstance(game, dict):
        raise ContractError("game must be a table")
    require_keys(game, GAME_KEYS, "game")
    game_type = game.get("type")
    if not isinstance(game_type, str) or game_type not in {"quiz", "quest"}:
        raise ContractError("game.type must be `quiz` or `quest`")
    optional_text(game, "title", "game")
    optional_text(game, "summary", "game")
    optional_text(game, "start_scene", "game")

    scenes = table_list(config, "scenes")
    mechanics = table_list(config, "mechanics")
    art = table_list(config, "art")
    scene_ids = collect_ids(scenes, "scene")
    mechanic_ids = collect_ids(mechanics, "mechanic")
    art_ids = collect_ids(art, "art")

    start_scene = game.get("start_scene")
    if scenes and start_scene is None:
        raise ContractError("game.start_scene is required when scenes are defined")
    if start_scene is not None:
        require_reference(start_scene, scene_ids, "game.start_scene")

    for index, scene in enumerate(scenes):
        location = f"scenes[{index}]"
        require_keys(scene, SCENE_KEYS, location)
        require_text(scene.get("title"), f"{location}.title")
        require_text(scene.get("kind"), f"{location}.kind")
        optional_text(scene, "summary", location)
        for reference in string_list(scene, "next", location):
            require_reference(reference, scene_ids, f"{location}.next")
        for reference in string_list(scene, "mechanics", location):
            require_reference(reference, mechanic_ids, f"{location}.mechanics")
        for reference in string_list(scene, "art", location):
            require_reference(reference, art_ids, f"{location}.art")

    for index, mechanic in enumerate(mechanics):
        location = f"mechanics[{index}]"
        require_keys(mechanic, MECHANIC_KEYS, location)
        require_text(mechanic.get("summary"), f"{location}.summary")
        for key in ("inputs", "rules", "feedback"):
            string_list(mechanic, key, location)

    for index, item in enumerate(art):
        location = f"art[{index}]"
        require_keys(item, ART_KEYS, location)
        require_text(item.get("kind"), f"{location}.kind")
        require_text(item.get("summary"), f"{location}.summary")
        string_list(item, "requirements", location)

    return len(scenes), len(mechanics), len(art)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: validate_codequest.py /path/to/CODEQUEST.toml", file=sys.stderr)
        return 2
    path = Path(sys.argv[1])
    try:
        with path.open("rb") as source:
            counts = validate(tomllib.load(source))
    except (OSError, tomllib.TOMLDecodeError, ContractError) as error:
        print(f"invalid {path.name}: {error}", file=sys.stderr)
        return 1
    print(
        f"valid {path.name}: {counts[0]} scenes, "
        f"{counts[1]} mechanics, {counts[2]} art requirements"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
