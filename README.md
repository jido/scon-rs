# Serde Scon

**Serde is a framework for *ser*ializing and *de*serializing Rust data structures efficiently and generically.**

---

```toml
[dependencies]
serde_scon = "0.0.1"
```

You may be looking for:

- [Serde JSON API documentation](https://docs.rs/serde_json)
- [Serde API documentation](https://docs.rs/serde)
- [Detailed documentation about Serde](https://serde.rs/)
- [Setting up `#[derive(Serialize, Deserialize)]`](https://serde.rs/derive.html)
- [Release notes](https://github.com/serde-rs/json/releases)

Scon is a new format derived from JSON that uses human-readable text to
transmit data objects consisting of key-value pairs.

```go
{
    name"John Doe"
    age 43
    address{
        street"10 Downing Street"
        city London
    }
    phones[
        "+44 1234567"
        "+44 2345678"
    ]
}
```

There are three common ways that you might find yourself needing to work with
Scon data in Rust.

 - **As text data.** An unprocessed string of Scon data that you receive on an
   HTTP endpoint, read from a file, or prepare to send to a remote server.
 - **As an untyped or loosely typed representation.** Maybe you want to check
   that some Scon data is valid before passing it on, but without knowing the
   structure of what it contains. Or you want to do very basic manipulations
   like insert a key in a particular spot.
 - **As a strongly typed Rust data structure.** When you expect all or most of
   your data to conform to a particular structure and want to get real work done
   without Scon's loose nature tripping you up.

Serde Scon provides efficient, flexible, safe ways of converting data between
each of these representations.

## Operating on untyped Scon values

Any valid Scon data can be manipulated in the following recursive enum
representation. This data structure is [`serde_scon::Value`][value].

```rust
enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(Map<String, Value>),
}
```

A string of Scon data can be parsed into a `serde_scon::Value` by the
[`serde_scon::from_str`][from_str] function. There is also
[`from_slice`][from_slice] for parsing from a byte slice `&[u8]` and
[`from_reader`][from_reader] for parsing from any `io::Read` like a File or a
TCP stream.

<div align="right">
<a href="https://play.rust-lang.org/?edition=2018&gist=d69d8e3156d4bb81c4461b60b772ab72" target="_blank">
<img align="center" width="85" src="https://raw.githubusercontent.com/serde-rs/serde-rs.github.io/master/img/runtab.png">
</a>
</div>

```rust
use serde_scon::{Result, Value};

fn untyped_example() -> Result<()> {
    // Some Scon input data as a &str. Maybe this comes from the user.
    let data = r#"{
            name"John Doe"
            age 43
            phones[
                "+44 1234567"
                "+44 2345678"
            ]
        }"#;

    // Parse the string of data into serde_scon::Value.
    let v: Value = serde_scon::from_str(data)?;

    // Access parts of the data by indexing with square brackets.
    println!("Please call {} at the number {}", v["name"], v["phones"][0]);

    Ok(())
}
```

The result of square bracket indexing like `v["name"]` is a borrow of the data
at that index, so the type is `&Value`. A Scon map can be indexed with string
keys, while a Scon array can be indexed with integer keys. If the type of the
data is not right for the type with which it is being indexed, or if a map does
not contain the key being indexed, or if the index into a vector is out of
bounds, the returned element is `Value::Null`.

When a `Value` is printed, it is printed as a Scon string. So in the code above,
the output looks like `Please call "John Doe" at the number "+44 1234567"`. The
quotation marks appear because `v["name"]` is a `&Value` containing a Scon
string and its Scon representation is `"John Doe"`. Printing as a plain string
without quotation marks involves converting from a Scon string to a Rust string
with [`as_str()`] or avoiding the use of `Value` as described in the following
section.

[`as_str()`]: https://docs.rs/serde_json/1/serde_json/enum.Value.html#method.as_str

The `Value` representation is sufficient for very basic tasks but can be tedious
to work with for anything more significant. Error handling is verbose to
implement correctly, for example imagine trying to detect the presence of
unrecognized fields in the input data. The compiler is powerless to help you
when you make a mistake, for example imagine typoing `v["name"]` as `v["nmae"]`
in one of the dozens of places it is used in your code.

## Parsing Scon as strongly typed data structures

Serde provides a powerful way of mapping Scon data into Rust data structures
largely automatically.

<div align="right">
<a href="https://play.rust-lang.org/?edition=2018&gist=15cfab66d38ff8a15a9cf1d8d897ac68" target="_blank">
<img align="center" width="85" src="https://raw.githubusercontent.com/serde-rs/serde-rs.github.io/master/img/runtab.png">
</a>
</div>

```rust
use serde::{Deserialize, Serialize};
use serde_scon::Result;

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u8,
    phones: Vec<String>,
}

fn typed_example() -> Result<()> {
    // Some Scon input data as a &str. Maybe this comes from the user.
    let data = r#"{
            name"John Doe"
            age 43
            phones[
                "+44 1234567"
                "+44 2345678"
            ]
        }"#;

    // Parse the string of data into a Person object. This is exactly the
    // same function as the one that produced serde_scon::Value above, but
    // now we are asking it for a Person as output.
    let p: Person = serde_scon::from_str(data)?;

    // Do things just like with any other Rust data structure.
    println!("Please call {} at the number {}", p.name, p.phones[0]);

    Ok(())
}
```

This is the same `serde_scon::from_str` function as before, but this time we
assign the return value to a variable of type `Person` so Serde will
automatically interpret the input data as a `Person` and produce informative
error messages if the layout does not conform to what a `Person` is expected to
look like.

Any type that implements Serde's `Deserialize` trait can be deserialized this
way. This includes built-in Rust standard library types like `Vec<T>` and
`HashMap<K, V>`, as well as any structs or enums annotated with
`#[derive(Deserialize)]`.

Once we have `p` of type `Person`, our IDE and the Rust compiler can help us use
it correctly like they do for any other Rust code. The IDE can autocomplete
field names to prevent typos, which was impossible in the `serde_scon::Value`
representation. And the Rust compiler can check that when we write
`p.phones[0]`, then `p.phones` is guaranteed to be a `Vec<String>` so indexing
into it makes sense and produces a `String`.

The necessary setup for using Serde's derive macros is explained on the *[Using
derive]* page of the Serde site.

[Using derive]: https://serde.rs/derive.html

## Constructing Scon values

Serde Scon provides a [`scon!` macro][macro] to build `serde_scon::Value`
objects with very natural JSON syntax.

<div align="right">
<a href="https://play.rust-lang.org/?edition=2018&gist=6ccafad431d72b62e77cc34c8e879b24" target="_blank">
<img align="center" width="85" src="https://raw.githubusercontent.com/serde-rs/serde-rs.github.io/master/img/runtab.png">
</a>
</div>

```rust
use serde_scon::scon;

fn main() {
    // The type of `john` is `serde_scon::Value`
    let john = scon!({
        "name": "John Doe",
        "age": 43,
        "phones": [
            "+44 1234567",
            "+44 2345678"
        ]
    });

    println!("first phone number: {}", john["phones"][0]);

    // Convert to a string of Scon and print it out
    println!("{}", john.to_string());
}
```

The `Value::to_string()` function converts a `serde_scon::Value` into a `String`
of Scon text.

One neat thing about the `scon!` macro is that variables and expressions can be
interpolated directly into the Scon value as you are building it. Serde will
check at compile time that the value you are interpolating is able to be
represented as Scon.

<div align="right">
<a href="https://play.rust-lang.org/?edition=2018&gist=f9101a6e61dfc9e02c6a67f315ed24f2" target="_blank">
<img align="center" width="85" src="https://raw.githubusercontent.com/serde-rs/serde-rs.github.io/master/img/runtab.png">
</a>
</div>

```rust
let full_name = "John Doe";
let age_last_year = 42;

// The type of `john` is `serde_scon::Value`
let john = scon!({
    "name": full_name,
    "age": age_last_year + 1,
    "phones": [
        format!("+44 {}", random_phone())
    ]
});
```

This is amazingly convenient, but we have the problem we had before with
`Value`: the IDE and Rust compiler cannot help us if we get it wrong. Serde Scon
provides a better way of serializing strongly-typed data structures into Scon
text.

## Creating Scon by serializing data structures

A data structure can be converted to a Scon string by
[`serde_scon::to_string`][to_string]. There is also
[`serde_scon::to_vec`][to_vec] which serializes to a `Vec<u8>` and
[`serde_scon::to_writer`][to_writer] which serializes to any `io::Write`
such as a File or a TCP stream.

<div align="right">
<a href="https://play.rust-lang.org/?edition=2018&gist=3472242a08ed2ff88a944f2a2283b0ee" target="_blank">
<img align="center" width="85" src="https://raw.githubusercontent.com/serde-rs/serde-rs.github.io/master/img/runtab.png">
</a>
</div>

```rust
use serde::{Deserialize, Serialize};
use serde_scon::Result;

#[derive(Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
}

fn print_an_address() -> Result<()> {
    // Some data structure.
    let address = Address {
        street: "10 Downing Street".to_owned(),
        city: "London".to_owned(),
    };

    // Serialize it to a Scon string.
    let j = serde_scon::to_string(&address)?;

    // Print, write to a file, or send to an HTTP server.
    println!("{}", j);

    Ok(())
}
```

Any type that implements Serde's `Serialize` trait can be serialized this way.
This includes built-in Rust standard library types like `Vec<T>` and `HashMap<K,
V>`, as well as any structs or enums annotated with `#[derive(Serialize)]`.

## Performance

It is fast. You should expect in the ballpark of 500 to 1000 megabytes per
second deserialization and 600 to 900 megabytes per second serialization,
depending on the characteristics of your data. This is competitive with the
fastest C and C++ JSON libraries or even 30% faster for many use cases.
Benchmarks live in the [serde-rs/json-benchmark] repo.

[serde-rs/json-benchmark]: https://github.com/serde-rs/json-benchmark

## Getting help

Serde is one of the most widely used Rust libraries, so any place that
Rustaceans congregate will be able to help you out. For chat, consider trying
the [#rust-questions] or [#rust-beginners] channels of the unofficial community
Discord (invite: <https://discord.gg/rust-lang-community>), the [#rust-usage] or
[#beginners] channels of the official Rust Project Discord (invite:
<https://discord.gg/rust-lang>), or the [#general][zulip] stream in Zulip. For
asynchronous, consider the [\[rust\] tag on StackOverflow][stackoverflow], the
[/r/rust] subreddit which has a pinned weekly easy questions post, or the Rust
[Discourse forum][discourse]. It's acceptable to file a support issue in this
repo, but they tend not to get as many eyes as any of the above and may get
closed without a response after some time.

[#rust-questions]: https://discord.com/channels/273534239310479360/274215136414400513
[#rust-beginners]: https://discord.com/channels/273534239310479360/273541522815713281
[#rust-usage]: https://discord.com/channels/442252698964721669/443150878111694848
[#beginners]: https://discord.com/channels/442252698964721669/448238009733742612
[zulip]: https://rust-lang.zulipchat.com/#narrow/stream/122651-general
[stackoverflow]: https://stackoverflow.com/questions/tagged/rust
[/r/rust]: https://www.reddit.com/r/rust
[discourse]: https://users.rust-lang.org

## No-std support

As long as there is a memory allocator, it is possible to use serde_scon without
the rest of the Rust standard library. Disable the default "std" feature and
enable the "alloc" feature:

```toml
[dependencies]
serde_scon = { version = "0.0.1", default-features = false, features = ["alloc"] }
```

For Scon support in Serde without a memory allocator, please see the
[`serde-json-core`] crate.

[`serde-json-core`]: https://github.com/rust-embedded-community/serde-json-core

[value]: https://docs.rs/serde_json/1/serde_json/value/enum.Value.html
[from_str]: https://docs.rs/serde_json/1/serde_json/de/fn.from_str.html
[from_slice]: https://docs.rs/serde_json/1/serde_json/de/fn.from_slice.html
[from_reader]: https://docs.rs/serde_json/1/serde_json/de/fn.from_reader.html
[to_string]: https://docs.rs/serde_json/1/serde_json/ser/fn.to_string.html
[to_vec]: https://docs.rs/serde_json/1/serde_json/ser/fn.to_vec.html
[to_writer]: https://docs.rs/serde_json/1/serde_json/ser/fn.to_writer.html
[macro]: https://docs.rs/serde_json/1/serde_json/macro.json.html

<br>

### License

Licensed under <a href="LICENSE-MIT">MIT license</a>.

<br>

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, shall
be licensed as above, without any additional terms or conditions.
