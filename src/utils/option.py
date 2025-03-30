from typing import Generic, TypeVar, Union

T = TypeVar("T")

class Option(Generic[T]):
    """
    RustのOption型を模した、値が存在するかしないかを扱うためのラッパークラス。

    Attributes:
        _value (Union[T, None]): ラップされた値。Noneの場合は値が存在しないことを意味する。
    """
    def __init__(self, value: Union[T, None]):
        self._value = value

    def is_some(self) -> bool:
        """
        値が存在する（Noneでない）かどうかを判定する。

        Returns:
            bool: 値が存在する場合はTrue。
        """
        return self._value is not None

    def is_none(self) -> bool:
        """
        値が存在しない（Noneである）かどうかを判定する。

        Returns:
            bool: 値が存在しない場合はTrue。
        """
        return self._value is None

    def unwrap(self) -> T:
        """
        値を取り出す。値が存在しない場合は例外を投げる。

        Returns:
            T: ラップされた値。

        Raises:
            ValueError: 値がNoneの場合。
        """
        if self._value is None:
            raise ValueError("Tried to unwrap a None value")
        return self._value

    def unwrap_or(self, default: T) -> T:
        """
        値が存在しない場合に指定されたデフォルト値を返す。

        Args:
            default (T): デフォルト値。

        Returns:
            T: 実際の値またはデフォルト値。
        """
        return self._value if self._value is not None else default

def Some(value: T) -> Option[T]:
    """
    値を持つOptionを作成する。

    Args:
        value (T): ラップする値。

    Returns:
        Option[T]: 値を持つOption。
    """
    return Option(value)

def None_() -> Option[T]:
    """
    値を持たないOptionを作成する。

    Returns:
        Option[T]: Noneを持つOption。
    """
    return Option(None)
