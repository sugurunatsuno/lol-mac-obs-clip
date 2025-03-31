# tests/test_option.py

from utils.option import Option, Some, None_

def test_some_value():
    value = Some("akari")
    assert value.is_some()
    assert not value.is_none()
    assert value.unwrap() == "akari"

def test_none_value():
    value = None_()
    assert value.is_none()
    assert not value.is_some()

def test_unwrap_raises():
    value = None_()
    try:
        value.unwrap()
        assert False, "Expected ValueError"
    except ValueError as e:
        assert str(e) == "Tried to unwrap a None value"

def test_unwrap_or():
    value = None_()
    assert value.unwrap_or("fallback") == "fallback"
