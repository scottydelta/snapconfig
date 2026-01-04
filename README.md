<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&color=0:3498db,100:9b59b6&height=150&section=header&text=snapconfig&fontSize=42&fontColor=ffffff&animation=fadeIn&fontAlignY=30" alt="snapconfig banner" width="100%">
</p>

<p align="center">
  <strong>Superfast config loader for Python, powered by Rust + rkyv.<br/> See benchmarks below.</strong>
</p>

<p align="center">
  <a href="https://github.com/scottydelta/snapconfig"><img src="https://img.shields.io/badge/GitHub-snapconfig-white?style=flat-square&logo=github&logoColor=black" alt="GitHub"></a>
  <a href="https://pypi.org/project/snapconfig/"><img src="https://img.shields.io/pypi/v/snapconfig?style=flat-square&logo=pypi&logoColor=white&label=PyPI" alt="PyPI"></a>
  <img src="https://img.shields.io/badge/python-3.9+-blue?style=flat-square&logo=python&logoColor=white" alt="Python 3.9+">
  <a href="https://github.com/scottydelta/snapconfig/blob/master/LICENSE"><img src="https://img.shields.io/github/license/scottydelta/snapconfig?style=flat-square" alt="License"></a>
  <a href="https://github.com/scottydelta/snapconfig/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/scottydelta/snapconfig/ci.yml?branch=master&style=flat-square&logo=github&label=CI" alt="CI"></a>
</p>

## What it does

- Parses JSON / YAML / TOML / INI / .env, compiles once, then memory‑maps the cache
- Zero-copy reads via Rust [rkyv](https://rkyv.org/) + `mmap`, so repeated loads stay fast and page-shared across processes
- Dict-like access in Python (`[]`, `.get`, `in`, `len`, iteration) plus dot-notation lookup
- Cache freshness check on load; caches are written atomically to avoid torn files

## Inspiration

- Need for superfast loading of large JSON files and other configs across processes/workers in AI workflow orchestrators. 
- [uv](https://github.com/astral-sh/uv) package manager, which uses rkyv to deserialize cached data without copying.

## Installation

```bash
pip install snapconfig
```

## Quick start

```python
import snapconfig

# Load any config format, automatically cached on first load
config = snapconfig.load("config.json")
config = snapconfig.load("config.yaml")
config = snapconfig.load("pyproject.toml")
config = snapconfig.load("settings.ini")

# Access values like a dict
db_host = config["database"]["host"]
db_port = config.get("database.port")  # dot notation supported

# Load .env files
env = snapconfig.load_env(".env")
snapconfig.load_dotenv(".env")  # populates os.environ
```

## How it works

```
First load:    config.json → parse → compile → config.json.snapconfig (cached)
                                                    ↓
Subsequent:                              mmap() → zero-copy access (~30µs)
```

1. **First load**: Parses source file and compiles to optimized binary cache
2. **Subsequent loads**: Memory-maps the cache file for instant zero-copy access

The cache file is automatically regenerated when the source file changes.

## Benchmarks (local run)

<p align="center">
  <img alt="Benchmark chart" src="https://raw.githubusercontent.com/scottydelta/snapconfig/master/docs/benchmark.png" width="800">
  <br>
  <img alt="Benchmark table" src="https://raw.githubusercontent.com/scottydelta/snapconfig/master/docs/benchmark_table.png" width="800">
</p>

Numbers from running `pipenv run python benchmark.py` on an M3 Pro (see `benchmark.py` for exact scenarios).

Takeaways:
- Cached reads stay in the low milliseconds down to tens of microseconds; big files benefit most.
- Cold loads beat YAML/ENV/TOML parsers; Python’s `json` still wins cold, but cached loads dominate.

### When snapconfig shines

- **CLI tools** that start frequently
- **Serverless functions** with cold starts
- **Multiple worker processes** reading the same config
- **Large config files** (package-lock.json, monorepo configs)

### Cold vs cached

| Scenario | What Happens | vs JSON | vs YAML/TOML/ENV |
|----------|--------------|---------|------------------|
| Cold (first load) | Parse + compile + write cache | Slower | **3-170x faster** |
| Cached (subsequent) | mmap() only | **3-5,000x faster** | **50-7,000x faster** |

Cold loads are slower than Python's `json` module (it's highly optimized C code), but faster than `pyyaml`, `tomllib`, and `python-dotenv`. The real payoff comes on cached loads.

## Supported formats

| Format | Extensions | Parser |
|--------|------------|--------|
| JSON   | `.json` | simd-json |
| YAML   | `.yaml`, `.yml` | serde_yaml |
| TOML   | `.toml` | toml |
| INI    | `.ini`, `.cfg`, `.conf` | rust-ini |
| dotenv | `.env`, `.env.*` | custom |

## API Reference

### Loading

```python
# Load with automatic caching (recommended)
config = snapconfig.load("config.json")
config = snapconfig.load("config.json", cache_path="custom.snapconfig")
config = snapconfig.load("config.json", force_recompile=True)

# Load directly from cache (skips freshness check)
config = snapconfig.load_compiled("config.json.snapconfig")

# Parse string content (no caching)
config = snapconfig.loads('{"key": "value"}', format="json")
config = snapconfig.loads("key: value", format="yaml")
```

### dotenv support

```python
# Load .env with caching
env = snapconfig.load_env(".env")
env = snapconfig.load_env(".env.production")

# Load into os.environ
count = snapconfig.load_dotenv(".env")
count = snapconfig.load_dotenv(".env", override_existing=True)

# Parse .env string
env = snapconfig.parse_env("KEY=value\nDEBUG=true")
```

### Cache management

```python
# Pre-compile config (e.g., during Docker build)
snapconfig.compile("config.json")
snapconfig.compile("config.json", "config.snapconfig")

# Check cache status
info = snapconfig.cache_info("config.json")
# {'source_exists': True, 'cache_exists': True, 'cache_fresh': True, ...}

# Clear cache
snapconfig.clear_cache("config.json")
```

### SnapConfig object

```python
config = snapconfig.load("config.json")

# Dict-like access
config["database"]["host"]
config["database"]["port"]

# Dot notation for nested access
config.get("database.host")
config.get("database.port", default=5432)

# Iteration
for key in config:
    print(key, config[key])

# Membership
"database" in config  # True

# Info
len(config)           # Number of keys
config.keys()         # List of keys
config.to_dict()      # Convert to Python dict
config.root_type()    # "object", "array", etc.
```

## Cross-process benefits

When multiple processes load the same cached config:

```python
# Worker 1, Worker 2, Worker 3...
config = snapconfig.load("config.json")  # All share same memory pages
```

The operating system's virtual memory system ensures all processes share the same physical memory pages via `mmap()`. This is particularly useful for:

- Prefect/Celery workers
- Gunicorn/uWSGI workers
- Multiprocessing pools
- Serverless function instances

## Pre-compilation

For production deployments, pre-compile configs during build:

```dockerfile
# Dockerfile
RUN python -c "import snapconfig; snapconfig.compile('config.json')"
```

```yaml
# CI/CD
- run: python -c "import snapconfig; snapconfig.compile('config.json')"
```

This ensures the first load in production is already cached.

## Acknowledgements

snapconfig is built on:

- [rkyv](https://rkyv.org/) - Zero-copy deserialization framework for Rust
- [PyO3](https://pyo3.rs/) - Rust bindings for Python
- [simd-json](https://github.com/simd-lite/simd-json) - SIMD-accelerated JSON parser
- [maturin](https://github.com/PyO3/maturin) - Build and publish Rust Python extensions

## License

MIT
