#!/usr/bin/env python3
# /// script
# dependencies = ["snapconfig", "pyyaml", "python-dotenv"]
# ///

import json
import os
import random
import shutil
import string
import time
from pathlib import Path

import snapconfig

try:
    import yaml
    HAS_YAML = True
except ImportError:
    HAS_YAML = False

try:
    from dotenv import dotenv_values
    HAS_DOTENV = True
except ImportError:
    HAS_DOTENV = False

try:
    import tomllib
    HAS_TOMLLIB = True
except ImportError:
    HAS_TOMLLIB = False


def random_string(length: int = 10) -> str:
    return ''.join(random.choices(string.ascii_letters + string.digits, k=length))


def format_size(bytes_size: int) -> str:
    for unit in ['B', 'KB', 'MB']:
        if bytes_size < 1024:
            return f"{bytes_size:.1f}{unit}"
        bytes_size /= 1024
    return f"{bytes_size:.1f}GB"


def format_time(seconds: float) -> str:
    if seconds < 0.001:
        return f"{seconds * 1_000_000:.1f}µs"
    elif seconds < 1:
        return f"{seconds * 1000:.2f}ms"
    return f"{seconds:.2f}s"


def gen_flat(n: int) -> dict:
    return {f"key_{i}": random_string(20) for i in range(n)}


def gen_nested(depth: int, breadth: int) -> dict:
    def build(d):
        if d >= depth:
            return {"val": random_string(10), "num": random.randint(0, 1000)}
        return {f"l{i}": build(d + 1) for i in range(breadth)}
    return {"root": build(0)}


def gen_array(n: int) -> dict:
    return {
        "items": [
            {"id": i, "name": random_string(15), "score": random.random(), "active": random.choice([True, False])}
            for i in range(n)
        ]
    }


def gen_package_lock(packages: int) -> dict:
    return {
        "name": "test-project",
        "version": "1.0.0",
        "lockfileVersion": 3,
        "packages": {
            f"node_modules/{random_string(12)}": {
                "version": f"{random.randint(1,20)}.{random.randint(0,99)}.{random.randint(0,99)}",
                "resolved": f"https://registry.npmjs.org/{random_string(12)}/-/{random_string(12)}-1.0.0.tgz",
                "integrity": f"sha512-{random_string(86)}",
                "dependencies": {random_string(10): f"^{random.randint(1,5)}.0.0" for _ in range(random.randint(0, 5))}
            }
            for _ in range(packages)
        }
    }


def gen_env(n: int) -> str:
    lines = []
    for i in range(n):
        val_type = random.choice(["str", "num", "bool"])
        if val_type == "str":
            val = random_string(30)
        elif val_type == "num":
            val = str(random.randint(1, 10000))
        else:
            val = random.choice(["true", "false"])
        lines.append(f"VAR_{i}={val}")
    return "\n".join(lines)


def gen_ini(sections: int, keys: int) -> str:
    lines = []
    for s in range(sections):
        lines.append(f"[section_{s}]")
        for k in range(keys):
            lines.append(f"key_{k} = {random_string(20)}")
        lines.append("")
    return "\n".join(lines)


def gen_toml(tables: int, keys: int) -> str:
    lines = []
    for t in range(tables):
        lines.append(f"[table_{t}]")
        for k in range(keys):
            lines.append(f'key_{k} = "{random_string(20)}"')
        lines.append("")
    return "\n".join(lines)


def benchmark(load_fn, path, iterations=10, warmup=2):
    for _ in range(warmup):
        load_fn(path)
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        load_fn(path)
        times.append(time.perf_counter() - start)
    return min(times), sum(times) / len(times)


def benchmark_cold(load_fn, path, clear_fn, iterations=5):
    times = []
    for _ in range(iterations):
        clear_fn()
        start = time.perf_counter()
        load_fn(path)
        times.append(time.perf_counter() - start)
    return min(times), sum(times) / len(times)


def run_benchmark(name, data, test_dir, fmt="json"):
    path = test_dir / f"{name}.{fmt}"

    if fmt == "json":
        with open(path, "w") as f:
            json.dump(data, f)
    elif fmt == "yaml" and HAS_YAML:
        with open(path, "w") as f:
            yaml.dump(data, f)
    elif fmt in ("env", "ini", "toml"):
        with open(path, "w") as f:
            f.write(data)
    else:
        return None

    file_size = os.path.getsize(path)
    snapconfig.clear_cache(str(path))

    if fmt == "json":
        pkg_name = "json"
        pkg_min, pkg_avg = benchmark(lambda p: json.load(open(p)), str(path))
    elif fmt == "yaml":
        pkg_name = "pyyaml"
        pkg_min, pkg_avg = benchmark(lambda p: yaml.safe_load(open(p)), str(path), iterations=5)
    elif fmt == "env" and HAS_DOTENV:
        pkg_name = "python-dotenv"
        pkg_min, pkg_avg = benchmark(lambda p: dotenv_values(p), str(path))
    elif fmt == "toml" and HAS_TOMLLIB:
        pkg_name = "tomllib"
        pkg_min, pkg_avg = benchmark(lambda p: tomllib.load(open(p, "rb")), str(path))
    else:
        pkg_name = "builtin"
        pkg_min, pkg_avg = benchmark(lambda p: open(p).read(), str(path))

    cold_min, cold_avg = benchmark_cold(
        lambda p: snapconfig.load(p),
        str(path),
        lambda: snapconfig.clear_cache(str(path)),
        iterations=3
    )

    snapconfig.load(str(path))
    cached_min, cached_avg = benchmark(snapconfig.load, str(path), iterations=20)

    speedup_cold = pkg_avg / cold_avg if cold_avg > 0 else 0
    speedup_cached = pkg_avg / cached_avg if cached_avg > 0 else 0

    return {
        "name": name,
        "fmt": fmt,
        "size": file_size,
        "pkg_name": pkg_name,
        "pkg_time": pkg_avg,
        "cold_time": cold_avg,
        "cached_time": cached_avg,
        "speedup_cold": speedup_cold,
        "speedup_cached": speedup_cached,
    }


def print_table(results):
    h = ["Test", "Size", "Package", "Pkg Time", "Cold (sc)", "Cached (sc)", "Cold Speedup", "Cached Speedup"]
    w = [16, 7, 14, 10, 10, 11, 12, 14]

    def row(cells):
        return "│ " + " │ ".join(f"{c:<{w[i]}}" if i < len(w) else c for i, c in enumerate(cells)) + " │"

    def sep(char, join):
        return join.join(char * (x + 2) for x in w)

    print("\n┌" + sep("─", "┬") + "┐")
    print(row(h))
    print("├" + sep("─", "┼") + "┤")

    for r in results:
        if r['speedup_cold'] < 1:
            cold_spd = f"-{1/r['speedup_cold']:.1f}x"
        else:
            cold_spd = f"+{r['speedup_cold']:.0f}x"
        cached_spd = f"+{r['speedup_cached']:.0f}x"
        cells = [
            r['name'],
            format_size(r['size']),
            r['pkg_name'],
            format_time(r['pkg_time']),
            format_time(r['cold_time']),
            format_time(r['cached_time']),
            cold_spd,
            cached_spd,
        ]
        print(row(cells))

    print("└" + sep("─", "┴") + "┘")
    print(f"\n  sc = snapconfig")
    print(f"  Cold   = first load (parse + write cache)")
    print(f"  Cached = subsequent loads (mmap only)")
    print(f"  +Nx = snapconfig N times faster, -Nx = snapconfig N times slower")


def main():
    test_dir = Path(".snapconfig_bench")
    test_dir.mkdir(exist_ok=True)

    print("=" * 85)
    print("SNAPCONFIG BENCHMARK")
    print("=" * 85)

    results = []

    tests = [
        # JSON - 1KB to 10MB
        ("json_1kb", gen_flat(30), "json"),
        ("json_10kb", gen_flat(300), "json"),
        ("json_100kb", gen_flat(3000), "json"),
        ("json_1mb", gen_flat(30000), "json"),
        ("json_5mb", gen_array(60000), "json"),
        ("json_10mb", gen_array(120000), "json"),
    ]

    if HAS_YAML:
        tests.extend([
            # YAML - 1KB to 100KB (YAML is slow, skip larger)
            ("yaml_1kb", gen_flat(30), "yaml"),
            ("yaml_10kb", gen_flat(300), "yaml"),
            ("yaml_100kb", gen_flat(3000), "yaml"),
        ])

    if HAS_DOTENV:
        tests.extend([
            # ENV - 1KB to 20KB
            ("env_1kb", gen_env(50), "env"),
            ("env_20kb", gen_env(1000), "env"),
        ])

    if HAS_TOMLLIB:
        tests.extend([
            # TOML - 5KB to 80KB
            ("toml_5kb", gen_toml(10, 20), "toml"),
            ("toml_80kb", gen_toml(50, 50), "toml"),
        ])

    print(f"\nRunning {len(tests)} benchmarks...\n")

    for name, data, fmt in tests:
        print(f"  {name}...", end=" ", flush=True)
        result = run_benchmark(name, data, test_dir, fmt)
        if result:
            results.append(result)
            print(f"{result['speedup_cached']:.0f}x faster (cached) than {result['pkg_name']}")
        else:
            print("skipped")

    print("\n" + "=" * 85)
    print("RESULTS")
    print("=" * 85)

    print_table(results)

    print("\n" + "=" * 85)
    print("SUMMARY BY FORMAT")
    print("=" * 85)

    for fmt, pkg in [("json", "json"), ("yaml", "pyyaml"), ("env", "python-dotenv"), ("toml", "tomllib")]:
        fmt_results = [r for r in results if r["fmt"] == fmt]
        if fmt_results:
            cold_speedups = [r["speedup_cold"] for r in fmt_results]
            cached_speedups = [r["speedup_cached"] for r in fmt_results]
            print(f"\n{fmt.upper()} vs {pkg}:")
            print(f"  Cold speedup:   {min(cold_speedups):.1f}x - {max(cold_speedups):.0f}x (avg {sum(cold_speedups)/len(cold_speedups):.0f}x)")
            print(f"  Cached speedup: {min(cached_speedups):.0f}x - {max(cached_speedups):.0f}x (avg {sum(cached_speedups)/len(cached_speedups):.0f}x)")

    print("\n" + "=" * 85)
    print("SUMMARY")
    print("=" * 85)

    cold_speedups = [r["speedup_cold"] for r in results]
    cached_speedups = [r["speedup_cached"] for r in results]
    cached_times = [r["cached_time"] for r in results]
    sizes = [r["size"] for r in results]
    large_results = [r for r in results if r["size"] > 1_000_000]

    print(f"""
Total benchmarks: {len(results)}
Size range: {format_size(min(sizes))} - {format_size(max(sizes))}

Cold speedup (first load vs package):
  Min:     {min(cold_speedups):,.1f}x
  Max:     {max(cold_speedups):,.0f}x
  Average: {sum(cold_speedups)/len(cold_speedups):,.1f}x

Cached speedup (subsequent loads vs package):
  Min:     {min(cached_speedups):,.0f}x
  Max:     {max(cached_speedups):,.0f}x
  Average: {sum(cached_speedups)/len(cached_speedups):,.0f}x

Cached load time:
  Min:     {format_time(min(cached_times))}
  Max:     {format_time(max(cached_times))}
  Average: {format_time(sum(cached_times)/len(cached_times))}
""")

    if large_results:
        large_cached = [r["speedup_cached"] for r in large_results]
        print(f"Large files (>1MB) cached speedup: {sum(large_cached)/len(large_cached):,.0f}x")

    print(f"""
Key insight:
  Cached loads take ~{format_time(sum(cached_times)/len(cached_times))} regardless of file size.
  This is just an mmap() call - no parsing!

Best for:
  - CLI tools that start frequently
  - Large config files (package-lock.json, etc.)
  - Serverless cold starts
""")

    shutil.rmtree(test_dir)


if __name__ == "__main__":
    main()
