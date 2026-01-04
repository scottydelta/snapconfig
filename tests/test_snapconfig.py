"""Test suite for snapconfig."""

import json
import os
import tempfile
import pytest
import snapconfig


@pytest.fixture
def temp_dir():
    with tempfile.TemporaryDirectory() as tmpdir:
        yield tmpdir


@pytest.fixture
def json_file(temp_dir):
    path = os.path.join(temp_dir, "test.json")
    data = {
        "string": "hello",
        "integer": 42,
        "float": 3.14,
        "boolean": True,
        "null": None,
        "array": [1, 2, 3],
        "nested": {
            "key": "value",
            "deep": {"level": 3}
        }
    }
    with open(path, "w") as f:
        json.dump(data, f)
    yield path
    snapconfig.clear_cache(path)


@pytest.fixture
def yaml_file(temp_dir):
    path = os.path.join(temp_dir, "test.yaml")
    content = """
string: hello
integer: 42
float: 3.14
boolean: true
null_value: null
array:
  - 1
  - 2
  - 3
nested:
  key: value
  deep:
    level: 3
"""
    with open(path, "w") as f:
        f.write(content)
    yield path
    snapconfig.clear_cache(path)


@pytest.fixture
def toml_file(temp_dir):
    path = os.path.join(temp_dir, "test.toml")
    content = """
[package]
name = "test-project"
version = "1.0.0"
authors = ["Alice", "Bob"]

[database]
host = "localhost"
port = 5432
enabled = true

[features]
list = ["a", "b", "c"]
"""
    with open(path, "w") as f:
        f.write(content)
    yield path
    snapconfig.clear_cache(path)


@pytest.fixture
def ini_file(temp_dir):
    path = os.path.join(temp_dir, "test.ini")
    content = """
[database]
host = localhost
port = 5432
enabled = true

[cache]
type = redis
ttl = 3600
"""
    with open(path, "w") as f:
        f.write(content)
    yield path
    snapconfig.clear_cache(path)


@pytest.fixture
def env_file(temp_dir):
    path = os.path.join(temp_dir, ".env")
    content = """
# Database settings
DATABASE_URL=postgres://localhost:5432/mydb
DB_PORT=5432

# Feature flags
DEBUG=true
VERBOSE=false

# API
API_KEY="sk-secret-key"
TIMEOUT=30.5

# Export syntax
export EXPORTED_VAR=exported_value
"""
    with open(path, "w") as f:
        f.write(content)
    yield path
    snapconfig.clear_cache(path)


class TestJSON:
    def test_load_json(self, json_file):
        config = snapconfig.load(json_file)
        assert config["string"] == "hello"
        assert config["integer"] == 42
        assert config["boolean"] is True

    def test_json_types(self, json_file):
        config = snapconfig.load(json_file)
        assert isinstance(config["string"], str)
        assert isinstance(config["integer"], int)
        assert isinstance(config["float"], float)
        assert isinstance(config["boolean"], bool)
        assert config["null"] is None

    def test_json_array(self, json_file):
        config = snapconfig.load(json_file)
        assert config["array"] == [1, 2, 3]
        assert config["array"][0] == 1
        assert config["array"][2] == 3

    def test_json_nested(self, json_file):
        config = snapconfig.load(json_file)
        assert config["nested"]["key"] == "value"
        assert config["nested"]["deep"]["level"] == 3

    def test_json_get_dotted(self, json_file):
        config = snapconfig.load(json_file)
        assert config.get("nested.key") == "value"
        assert config.get("nested.deep.level") == 3

    def test_json_keys(self, json_file):
        config = snapconfig.load(json_file)
        keys = config.keys()
        assert "string" in keys
        assert "integer" in keys
        assert "nested" in keys

    def test_json_len(self, json_file):
        config = snapconfig.load(json_file)
        assert len(config) == 7

    def test_json_contains(self, json_file):
        config = snapconfig.load(json_file)
        assert "string" in config
        assert "nonexistent" not in config

    def test_json_to_dict(self, json_file):
        config = snapconfig.load(json_file)
        d = config.to_dict()
        assert isinstance(d, dict)
        assert d["string"] == "hello"
        assert d["nested"]["key"] == "value"


class TestYAML:
    def test_load_yaml(self, yaml_file):
        config = snapconfig.load(yaml_file)
        assert config["string"] == "hello"
        assert config["integer"] == 42

    def test_yaml_types(self, yaml_file):
        config = snapconfig.load(yaml_file)
        assert isinstance(config["integer"], int)
        assert isinstance(config["float"], float)
        assert isinstance(config["boolean"], bool)

    def test_yaml_nested(self, yaml_file):
        config = snapconfig.load(yaml_file)
        assert config["nested"]["deep"]["level"] == 3


class TestTOML:
    def test_load_toml(self, toml_file):
        config = snapconfig.load(toml_file)
        assert config["package"]["name"] == "test-project"
        assert config["package"]["version"] == "1.0.0"

    def test_toml_types(self, toml_file):
        config = snapconfig.load(toml_file)
        assert isinstance(config["database"]["port"], int)
        assert config["database"]["port"] == 5432
        assert isinstance(config["database"]["enabled"], bool)
        assert config["database"]["enabled"] is True

    def test_toml_arrays(self, toml_file):
        config = snapconfig.load(toml_file)
        assert config["package"]["authors"] == ["Alice", "Bob"]
        assert config["features"]["list"] == ["a", "b", "c"]


class TestINI:
    def test_load_ini(self, ini_file):
        config = snapconfig.load(ini_file)
        assert config["database"]["host"] == "localhost"

    def test_ini_types(self, ini_file):
        config = snapconfig.load(ini_file)
        assert isinstance(config["database"]["port"], int)
        assert config["database"]["port"] == 5432
        assert isinstance(config["database"]["enabled"], bool)
        assert config["cache"]["ttl"] == 3600

    def test_ini_sections(self, ini_file):
        config = snapconfig.load(ini_file)
        assert "database" in config
        assert "cache" in config

    def test_ini_null_and_bool_variants(self, temp_dir):
        path = os.path.join(temp_dir, "null.ini")
        content = """
[section]
empty =
nil = nil
TRUE = TRUE
False = False
"""
        with open(path, "w") as f:
            f.write(content)
        config = snapconfig.load(path)
        assert config["section"]["empty"] == ""
        assert config["section"]["nil"] is None
        assert config["section"]["TRUE"] is True
        assert config["section"]["False"] is False
        snapconfig.clear_cache(path)


class TestEnv:
    def test_load_env(self, env_file):
        config = snapconfig.load_env(env_file)
        assert config["DATABASE_URL"] == "postgres://localhost:5432/mydb"

    def test_env_types(self, env_file):
        config = snapconfig.load_env(env_file)
        assert isinstance(config["DB_PORT"], int)
        assert config["DB_PORT"] == 5432
        assert isinstance(config["DEBUG"], bool)
        assert config["DEBUG"] is True
        assert isinstance(config["TIMEOUT"], float)
        assert config["TIMEOUT"] == 30.5

    def test_env_quotes(self, env_file):
        config = snapconfig.load_env(env_file)
        assert config["API_KEY"] == "sk-secret-key"

    def test_env_export_prefix(self, env_file):
        config = snapconfig.load_env(env_file)
        assert config["EXPORTED_VAR"] == "exported_value"

    def test_env_null_variants(self, temp_dir):
        path = os.path.join(temp_dir, ".env.nulls")
        with open(path, "w") as f:
            f.write("EMPTY=\nNONE=None\nNIL=nil\nNULL=null\n")
        config = snapconfig.load_env(path)
        assert config["EMPTY"] == ""
        assert config["NONE"] is None
        assert config["NIL"] is None
        assert config["NULL"] is None
        snapconfig.clear_cache(path)

    def test_load_dotenv(self, env_file):
        for key in ["DATABASE_URL", "DEBUG", "DB_PORT"]:
            os.environ.pop(key, None)

        count = snapconfig.load_dotenv(env_file)
        assert count > 0
        assert os.environ.get("DATABASE_URL") == "postgres://localhost:5432/mydb"
        assert os.environ.get("DEBUG") == "true"

    def test_load_dotenv_no_override(self, env_file):
        os.environ["DATABASE_URL"] = "original"
        snapconfig.load_dotenv(env_file, override_existing=False)
        assert os.environ["DATABASE_URL"] == "original"

    def test_load_dotenv_override(self, env_file):
        os.environ["DATABASE_URL"] = "original"
        snapconfig.load_dotenv(env_file, override_existing=True)
        assert os.environ["DATABASE_URL"] == "postgres://localhost:5432/mydb"

    def test_parse_env(self):
        result = snapconfig.parse_env("FOO=bar\nNUM=42\nBOOL=true")
        assert result["FOO"] == "bar"
        assert result["NUM"] == 42
        assert result["BOOL"] is True


class TestCaching:
    def test_cache_created(self, json_file):
        snapconfig.clear_cache(json_file)
        assert not os.path.exists(f"{json_file}.snapconfig")
        snapconfig.load(json_file)
        assert os.path.exists(f"{json_file}.snapconfig")

    def test_cache_info(self, json_file):
        snapconfig.load(json_file)
        info = snapconfig.cache_info(json_file)
        assert info["source_exists"] is True
        assert info["cache_exists"] is True
        assert info["cache_fresh"] is True
        assert info["cache_size"] > 0

    def test_clear_cache(self, json_file):
        snapconfig.load(json_file)
        assert os.path.exists(f"{json_file}.snapconfig")
        result = snapconfig.clear_cache(json_file)
        assert result is True
        assert not os.path.exists(f"{json_file}.snapconfig")

    def test_force_recompile(self, json_file):
        snapconfig.load(json_file)
        mtime1 = os.path.getmtime(f"{json_file}.snapconfig")

        import time
        time.sleep(0.1)

        snapconfig.load(json_file, force_recompile=True)
        mtime2 = os.path.getmtime(f"{json_file}.snapconfig")
        assert mtime2 > mtime1

    def test_custom_cache_path(self, json_file, temp_dir):
        custom_cache = os.path.join(temp_dir, "custom.snapconfig")
        snapconfig.load(json_file, cache_path=custom_cache)
        assert os.path.exists(custom_cache)


class TestErrors:
    def test_file_not_found(self, temp_dir):
        with pytest.raises(Exception):
            snapconfig.load(os.path.join(temp_dir, "nonexistent.json"))

    def test_invalid_json(self, temp_dir):
        path = os.path.join(temp_dir, "invalid.json")
        with open(path, "w") as f:
            f.write("{invalid json}")
        with pytest.raises(Exception):
            snapconfig.load(path)

    def test_key_not_found(self, json_file):
        config = snapconfig.load(json_file)
        with pytest.raises(KeyError):
            _ = config["nonexistent_key"]

    def test_index_out_of_bounds(self, json_file):
        config = snapconfig.load(json_file)
        with pytest.raises(IndexError):
            _ = config["array"][100]


class TestCompile:
    def test_compile(self, json_file, temp_dir):
        output = os.path.join(temp_dir, "compiled.snapconfig")
        result = snapconfig.compile(json_file, output)
        assert result == output
        assert os.path.exists(output)

    def test_load_compiled(self, json_file, temp_dir):
        cache = os.path.join(temp_dir, "compiled.snapconfig")
        snapconfig.compile(json_file, cache)
        config = snapconfig.load_compiled(cache)
        assert config["string"] == "hello"


class TestPerformance:
    def test_cached_load_is_fast(self, json_file):
        import time

        snapconfig.clear_cache(json_file)
        start = time.perf_counter()
        snapconfig.load(json_file)
        first_load = time.perf_counter() - start

        times = []
        for _ in range(100):
            start = time.perf_counter()
            snapconfig.load(json_file)
            times.append(time.perf_counter() - start)

        avg_cached = sum(times) / len(times)
        assert avg_cached < first_load


class TestEdgeCases:
    def test_empty_object(self, temp_dir):
        path = os.path.join(temp_dir, "empty.json")
        with open(path, "w") as f:
            f.write("{}")
        config = snapconfig.load(path)
        assert len(config) == 0
        snapconfig.clear_cache(path)

    def test_empty_array_root(self, temp_dir):
        path = os.path.join(temp_dir, "array.json")
        with open(path, "w") as f:
            json.dump([1, 2, 3], f)
        config = snapconfig.load(path)
        assert config[0] == 1
        assert len(config) == 3
        snapconfig.clear_cache(path)

    def test_unicode(self, temp_dir):
        path = os.path.join(temp_dir, "unicode.json")
        with open(path, "w") as f:
            json.dump({"emoji": "🚀", "chinese": "中文", "accents": "éàü"}, f)
        config = snapconfig.load(path)
        assert config["emoji"] == "🚀"
        assert config["chinese"] == "中文"
        assert config["accents"] == "éàü"
        snapconfig.clear_cache(path)

    def test_large_numbers(self, temp_dir):
        path = os.path.join(temp_dir, "numbers.json")
        with open(path, "w") as f:
            json.dump({
                "large_int": 9007199254740992,
                "negative": -9007199254740992,
                "float": 1.7976931348623157e+308
            }, f)
        config = snapconfig.load(path)
        assert config["large_int"] == 9007199254740992
        assert config["negative"] == -9007199254740992
        snapconfig.clear_cache(path)

    def test_special_strings(self, temp_dir):
        path = os.path.join(temp_dir, "special.json")
        with open(path, "w") as f:
            json.dump({
                "newline": "line1\nline2",
                "tab": "col1\tcol2",
                "quote": 'say "hello"',
                "backslash": "path\\to\\file"
            }, f)
        config = snapconfig.load(path)
        assert config["newline"] == "line1\nline2"
        assert config["tab"] == "col1\tcol2"
        assert config["quote"] == 'say "hello"'
        snapconfig.clear_cache(path)

    def test_array_root_root_type_and_errors(self, temp_dir):
        path = os.path.join(temp_dir, "array_root.json")
        with open(path, "w") as f:
            json.dump([{"id": 1}, {"id": 2}], f)
        config = snapconfig.load(path)
        assert config.root_type() == "array"
        with pytest.raises(TypeError):
            config.get("0.id.more")
        snapconfig.clear_cache(path)

    def test_object_keys_sorted(self, temp_dir):
        path = os.path.join(temp_dir, "unordered.json")
        with open(path, "w") as f:
            json.dump({"b": 1, "a": 2, "c": 3}, f)
        config = snapconfig.load(path)
        keys = list(config.keys())
        assert keys == ["a", "b", "c"]
        snapconfig.clear_cache(path)
