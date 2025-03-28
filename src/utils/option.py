from typing import Generic, TypeVar, Union

T = TypeVar("T")

class Option(Generic[T]):
    def __init__(self, value: Union[T, None]):
        self._value = value

    def is_some(self) -> bool:
        return self._value is not None

    def is_none(self) -> bool:
        return self._value is None

    def unwrap(self) -> T:
        if self._value is None:
            raise ValueError("Tried to unwrap a None value")
        return self._value

    def unwrap_or(self, default: T) -> T:
        return self._value if self._value is not None else default

def Some(value: T) -> Option[T]:
    return Option(value)

def None_() -> Option[T]:
    return Option(None)
