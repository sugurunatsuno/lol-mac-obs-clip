from typing import Generic, TypeVar, Union

T = TypeVar("T")
E = TypeVar("E")

class Result(Generic[T, E]):
    """
    RustのResult型を模した、成功（Ok）または失敗（Err）の状態を表すラッパークラス。

    Attributes:
        _is_ok (bool): Okの状態かどうかを示すフラグ。
        _value (Union[T, E]): 成功時の値またはエラー情報。
    """
    def __init__(self, is_ok: bool, value: Union[T, E]):
        self._is_ok = is_ok
        self._value = value

    def is_ok(self) -> bool:
        """
        Okの状態かどうかを判定します。

        Returns:
            bool: OkであればTrue。
        """
        return self._is_ok

    def is_err(self) -> bool:
        """
        Errの状態かどうかを判定します。

        Returns:
            bool: ErrであればTrue。
        """
        return not self._is_ok

    def unwrap(self) -> T:
        """
        Okの値を取り出します。Errの場合は例外を投げます。

        Returns:
            T: Okの値。

        Raises:
            ValueError: Errだった場合。
        """
        if not self._is_ok:
            raise ValueError(f"Called unwrap on Err: {self._value}")
        return self._value

    def unwrap_err(self) -> E:
        """
        Errの値を取り出します。Okの場合は例外を投げます。

        Returns:
            E: Errの値。

        Raises:
            ValueError: Okだった場合。
        """
        if self._is_ok:
            raise ValueError(f"Called unwrap_err on Ok: {self._value}")
        return self._value

    def unwrap_or(self, default: T) -> T:
        """
        Okであればその値を返し、Errであればデフォルト値を返します。

        Args:
            default (T): Err時に返すデフォルト値。

        Returns:
            T: Okの値またはデフォルト値。
        """
        return self._value if self._is_ok else default

def Ok(value: T) -> Result[T, E]:
    """
    OkのResultを作成します。

    Args:
        value (T): 成功時の値。

    Returns:
        Result[T, E]: OkのResult。
    """
    return Result(True, value)

def Err(error: E) -> Result[T, E]:
    """
    ErrのResultを作成します。

    Args:
        error (E): エラーの情報。

    Returns:
        Result[T, E]: ErrのResult。
    """
    return Result(False, error)
