# tests/test_result.py

from utils.result import Ok, Err

def test_ok_result():
    result = Ok(42)
    assert result.is_ok()
    assert not result.is_err()
    assert result.unwrap() == 42

def test_err_result():
    result = Err("error!")
    assert result.is_err()
    assert not result.is_ok()

def test_unwrap_err_raises():
    result = Err("nope")
    try:
        result.unwrap()
        assert False, "Expected ValueError"
    except ValueError as e:
        assert "Called unwrap on Err" in str(e)

def test_unwrap_or_on_err():
    result = Err("fail")
    assert result.unwrap_or(100) == 100

def test_unwrap_or_on_ok():
    result = Ok(200)
    assert result.unwrap_or(999) == 200
