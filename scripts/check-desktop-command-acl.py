#!/usr/bin/env python3
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LIB = ROOT / 'apps/desktop/src-tauri/src/lib.rs'
BUILD = ROOT / 'apps/desktop/src-tauri/build.rs'
PERMISSIONS = ROOT / 'apps/desktop/src-tauri/permissions/editor.toml'
MAIN_CAPABILITY = ROOT / 'apps/desktop/src-tauri/capabilities/main-editor.json'
BENCHMARK_CAPABILITY = ROOT / 'apps/desktop/src-tauri/capabilities/renderer-benchmark.json'

BENCHMARK_ONLY = {
    'renderer_benchmark_environment',
    'persist_renderer_benchmark_evidence',
    'close_renderer_benchmark',
}


def fail(message: str) -> None:
    raise SystemExit(f'Desktop command ACL contract failed: {message}')


def permission_id(command: str) -> str:
    return 'allow-' + command.replace('_', '-')


def extract_handler_commands(text: str) -> list[str]:
    match = re.search(r'tauri::generate_handler!\[\s*(.*?)\s*\]\)', text, re.S)
    if not match:
        fail('could not locate tauri::generate_handler! command list')
    commands = re.findall(r'^\s*([a-z][a-z0-9_]*)\s*,?\s*$', match.group(1), re.M)
    if not commands:
        fail('generate_handler! command list is empty')
    return commands


def extract_manifest_commands(text: str) -> list[str]:
    match = re.search(r'AppManifest::new\(\)\.commands\(&\[\s*(.*?)\s*\]\)', text, re.S)
    if not match:
        fail('could not locate AppManifest::commands list')
    return re.findall(r'"([a-z][a-z0-9_]*)"', match.group(1))


def extract_permission_map(text: str) -> dict[str, str]:
    result: dict[str, str] = {}
    blocks = re.split(r'(?=^\[\[permission\]\]\s*$)', text, flags=re.M)
    for block in blocks:
        identifier = re.search(r'^identifier\s*=\s*"([^"]+)"\s*$', block, re.M)
        allow = re.search(r'^commands\.allow\s*=\s*\[\s*"([a-z][a-z0-9_]*)"\s*\]\s*$', block, re.M)
        if not identifier and not allow:
            continue
        if not identifier or not allow:
            fail('every editor permission block must contain one identifier and one allowed command')
        command = allow.group(1)
        if command in result:
            fail(f'duplicate explicit permission for {command}')
        result[command] = identifier.group(1)
    return result


def ensure_unique(label: str, values: list[str]) -> None:
    if len(values) != len(set(values)):
        fail(f'{label} contains duplicate command entries')


handler_commands = extract_handler_commands(LIB.read_text(encoding='utf-8'))
manifest_commands = extract_manifest_commands(BUILD.read_text(encoding='utf-8'))
ensure_unique('generate_handler!', handler_commands)
ensure_unique('AppManifest::commands', manifest_commands)

if set(handler_commands) != set(manifest_commands):
    missing = sorted(set(handler_commands) - set(manifest_commands))
    extra = sorted(set(manifest_commands) - set(handler_commands))
    fail(f'AppManifest mismatch; missing={missing}, extra={extra}')

permission_map = extract_permission_map(PERMISSIONS.read_text(encoding='utf-8'))
if set(permission_map) != set(handler_commands):
    missing = sorted(set(handler_commands) - set(permission_map))
    extra = sorted(set(permission_map) - set(handler_commands))
    fail(f'explicit permission inventory mismatch; missing={missing}, extra={extra}')

wrong_identifiers = {
    command: identifier
    for command, identifier in permission_map.items()
    if identifier != permission_id(command)
}
if wrong_identifiers:
    fail(f'permission identifiers do not match allow-<command>: {wrong_identifiers}')

main = json.loads(MAIN_CAPABILITY.read_text(encoding='utf-8'))
benchmark = json.loads(BENCHMARK_CAPABILITY.read_text(encoding='utf-8'))
main_permissions = set(main.get('permissions', []))
benchmark_permissions = set(benchmark.get('permissions', []))
expected_benchmark = {permission_id(command) for command in BENCHMARK_ONLY}
expected_main = {permission_id(command) for command in handler_commands if command not in BENCHMARK_ONLY}

if main_permissions != expected_main:
    fail(
        'main-editor capability mismatch; '
        f'missing={sorted(expected_main - main_permissions)}, '
        f'extra={sorted(main_permissions - expected_main)}'
    )
if benchmark_permissions != expected_benchmark:
    fail(
        'renderer-benchmark capability mismatch; '
        f'missing={sorted(expected_benchmark - benchmark_permissions)}, '
        f'extra={sorted(benchmark_permissions - expected_benchmark)}'
    )
if main_permissions & benchmark_permissions:
    fail(f'benchmark-only permissions leaked into main editor: {sorted(main_permissions & benchmark_permissions)}')

print(
    'Desktop command ACL contract OK: '
    f'{len(handler_commands)} registered commands, '
    f'{len(expected_main)} main-editor commands, '
    f'{len(expected_benchmark)} benchmark-only commands.'
)
