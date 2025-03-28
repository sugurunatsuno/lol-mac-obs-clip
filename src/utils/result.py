from typing import Generic, TypeVar, Union

T = TypeVar("T")
E = TypeVar("E")

class Result(Generic[T, E]):
    def __init__(self, is_ok: bool, value: Union[T, E]):
        self._is_ok = is_ok
        self._value = value

    def is_ok(self) -> bool:
        return self._is_ok

    def is_err(self) -> bool:
        return not self._is_ok

    def unwrap(self) -> T:
        if not self._is_ok:
            raise ValueError(f"Called unwrap on Err: {self._value}")
        return self._value

    def unwrap_err(self) -> E:
        if self._is_ok:
            raise ValueError(f"Called unwrap_err on Ok: {self._value}")
        return self._value

    def unwrap_or(self, default: T) -> T:
        return self._value if self._is_ok else default

def Ok(value: T) -> Result[T, E]:
    return Result(True, value)

def Err(error: E) -> Result[T, E]:
    return Result(False, error)
