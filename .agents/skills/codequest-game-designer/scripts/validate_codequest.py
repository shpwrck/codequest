#!/usr/bin/env python3
"""Fast authoring validation for CODEQUEST.toml schemas v1 and v2.

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
SCENE_KEYS = {
    "id",
    "title",
    "kind",
    "summary",
    "mechanics",
    "art",
    "next",
    "handler",
    "transitions",
}
TRANSITION_KEYS = {"signal", "target", "after_ticks"}
MECHANIC_KEYS = {"id", "summary", "inputs", "rules", "feedback"}
ART_KEYS = {"id", "kind", "summary", "template", "requirements"}
VISUAL_TEMPLATES = {
    "oracle-chronicle",
    "oracle-awakening",
    "oracle-title",
    "oracle-menu",
    "oracle-atelier",
    "oracle-hero",
    "oracle-sanctum",
    "oracle-trial",
    "oracle-ascension",
    "oracle-aftermath",
    "oracle-progression",
}
HANDLER_SIGNALS = {
    "repository-credits": {"continue", "elapsed"},
    "opening-fanfare": {"continue", "elapsed"},
    "title": {"continue"},
    "quiz-menu": {"new-run", "back"},
    "character-creation": {"hero-ready", "back"},
    "oracle": {"questions-ready", "back"},
    "concept-quiz": {"needs-question", "batch-complete", "hearts-empty", "back"},
    "level-up": {"questions-ready", "needs-question"},
    "game-over": {"replay"},
    "quest-select": {"quest-selected", "back"},
    "battle": {"victory", "defeat"},
    "victory": {"continue"},
    "defeat": {"continue"},
}
SHARED_HANDLERS = {"repository-credits", "opening-fanfare", "title"}
GAME_HANDLERS = {
    "quiz": {
        "quiz-menu",
        "character-creation",
        "oracle",
        "concept-quiz",
        "level-up",
        "game-over",
    },
    "quest": {"quest-select", "battle", "victory", "defeat"},
}


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


def nested_table_list(table: dict[str, Any], key: str, location: str) -> list[dict[str, Any]]:
    value = table.get(key, [])
    if not isinstance(value, list) or any(not isinstance(item, dict) for item in value):
        raise ContractError(f"{location}.{key} must be an array of tables")
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
    if type(schema_version) is not int or schema_version not in {1, 2}:
        raise ContractError("schema_version must equal 1 or 2")

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
    if schema_version == 2 and not scenes:
        raise ContractError("schema_version 2 requires at least one scene")

    graph: dict[str, list[str]] = {}

    for index, scene in enumerate(scenes):
        location = f"scenes[{index}]"
        require_keys(scene, SCENE_KEYS, location)
        require_text(scene.get("title"), f"{location}.title")
        require_text(scene.get("kind"), f"{location}.kind")
        optional_text(scene, "summary", location)
        next_scenes = string_list(scene, "next", location)
        transitions = nested_table_list(scene, "transitions", location)
        if schema_version == 1:
            if "handler" in scene or transitions:
                raise ContractError(f"{location} runtime fields require schema_version 2")
            for reference in next_scenes:
                require_reference(reference, scene_ids, f"{location}.next")
        else:
            if next_scenes:
                raise ContractError(f"{location}.next is a schema_version 1 field")
            handler = scene.get("handler")
            if not isinstance(handler, str) or handler not in HANDLER_SIGNALS:
                raise ContractError(f"{location}.handler is not a supported scene handler")
            if handler not in SHARED_HANDLERS | GAME_HANDLERS[game_type]:
                raise ContractError(
                    f"{location}.handler `{handler}` is not available for `{game_type}` games"
                )
            seen_signals: set[str] = set()
            graph[scene["id"]] = []
            for transition_index, transition in enumerate(transitions):
                transition_location = f"{location}.transitions[{transition_index}]"
                require_keys(transition, TRANSITION_KEYS, transition_location)
                signal = require_text(transition.get("signal"), f"{transition_location}.signal")
                target = require_text(transition.get("target"), f"{transition_location}.target")
                if signal not in HANDLER_SIGNALS[handler]:
                    raise ContractError(
                        f"{transition_location}.signal `{signal}` is not emitted by `{handler}`"
                    )
                if signal in seen_signals:
                    raise ContractError(f"{location} has duplicate `{signal}` transitions")
                seen_signals.add(signal)
                require_reference(target, scene_ids, f"{transition_location}.target")
                graph[scene["id"]].append(target)
                after_ticks = transition.get("after_ticks")
                if after_ticks is not None and (
                    type(after_ticks) is not int or after_ticks < 0
                ):
                    raise ContractError(
                        f"{transition_location}.after_ticks must be a non-negative integer"
                    )
                if signal == "elapsed" and after_ticks is None:
                    raise ContractError(
                        f"{transition_location}.after_ticks is required for `elapsed`"
                    )
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
        if "template" in item:
            template = require_text(item["template"], f"{location}.template")
            if template not in VISUAL_TEMPLATES:
                raise ContractError(f"{location}.template `{template}` is not built in")
        string_list(item, "requirements", location)

    if schema_version == 2:
        reachable: set[str] = set()
        pending = [start_scene]
        while pending:
            scene_id = pending.pop()
            if scene_id in reachable:
                continue
            reachable.add(scene_id)
            pending.extend(graph[scene_id])
        unreachable = sorted(scene_ids - reachable)
        if unreachable:
            raise ContractError(f"unreachable scenes: {', '.join(unreachable)}")

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
