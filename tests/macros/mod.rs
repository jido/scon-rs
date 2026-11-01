#![allow(unused_macro_rules)]

macro_rules! scon_str {
    ([]) => {
        "[]"
    };
    ([ $e0:tt $(, $e:tt)* $(,)? ]) => {
        concat!("[",
            scon_str!($e0),
            $(",", scon_str!($e),)*
        "]")
    };
    ({}) => {
        "{}"
    };
    ({ $k0:tt : $v0:tt $(, $k:tt : $v:tt)* $(,)? }) => {
        concat!("{",
            stringify!($k0), scon_str!($v0),
            $(",", stringify!($k), scon_str!($v),)*
        "}")
    };
    (true) => {
        "*"
    };
    (false) => {
        "~"
    };
    (null) => {
        "/"
    };
    ((($other:tt))) => {
        concat!("`", stringify($other))
    };
    (($other:tt)) => {
        $other
    };
    ($other:tt) => {
        stringify!($other)
    };
}

macro_rules! pretty_str {
    ($scon:tt) => {
        pretty_str_impl!("", $scon)
    };
}

macro_rules! pretty_str_impl {
    ($indent:expr, []) => {
        "[]"
    };
    ($indent:expr, [ $e0:tt $(, $e:tt)* $(,)? ]) => {
        concat!("[\n ",
            $indent, pretty_str_impl!(concat!(" ", $indent), $e0),
            $("\n ", $indent, pretty_str_impl!(concat!(" ", $indent), $e),)*
        "\n", $indent, "]")
    };
    ($indent:expr, {}) => {
        "{}"
    };
    ($indent:expr, { $k0:tt : $v0:tt $(, $k:tt : $v:tt)* $(,)? }) => {
        concat!("{\n ",
            $indent, stringify!($k0), pretty_str_impl!(concat!(" ", $indent), $v0),
            $("\n ", $indent, stringify!($k), pretty_str_impl!(concat!(" ", $indent), $v),)*
        "\n", $indent, "}")
    };
    ($indent:expr, true) => {
        "*"
    };
    ($indent:expr, false) => {
        "~"
    };
    ($indent:expr, null) => {
        "/"
    };
    ($indent:expr, (($other:tt))) => {
        concat!("`", stringify!($other))
    };
    ($indent:expr, ($other:tt)) => {
        $other
    };
    ($indent:expr, $other:tt) => {
        stringify!($other)
    };
}
