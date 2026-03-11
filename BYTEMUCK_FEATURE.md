# Bytemuck Feature Implementation

## Overview

This document describes the implementation of the `bytemuck` feature in the `orca_whirlpools_client` crate, which enables efficient zero-copy deserialization of on-chain account data.

## Problem Statement

The on-chain Whirlpool program stores account data in a **packed** format (no alignment padding between fields). However, the default Rust struct representation uses **C alignment** with padding for performance. This creates two conflicting needs:

1. **Performance**: Normal C alignment allows faster field access
2. **Correctness**: Packed layout is required for zero-copy deserialization from on-chain data

## Solution

Implemented a feature-gated approach that allows users to choose between:

- **Default Borsh deserialization** (without `bytemuck` feature): Uses `#[repr(C)]` with normal alignment - standard behavior
- **Zero-copy bytemuck deserialization** (with `bytemuck` feature): Uses `#[repr(C, packed)]` for exact on-chain layout matching - opt-in for performance

## Implementation Details

### Cargo.toml Changes

```toml
[features]
default = ["core-types"]
bytemuck = ["dep:bytemuck"]

[dependencies]
bytemuck = { version = "^1.14", features = ["derive"], optional = true }
```

The `bytemuck` feature is **opt-in** - users must explicitly enable it when they need zero-copy deserialization for on-chain data.

### Struct Modifications

All account and type structs now use conditional compilation:

```rust
#[cfg_attr(not(feature = "bytemuck"), derive(BorshSerialize, BorshDeserialize))]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "bytemuck", repr(C, packed))]
#[cfg_attr(not(feature = "bytemuck"), repr(C))]
pub struct Whirlpool {
    // ... fields
}

#[cfg(feature = "bytemuck")]
unsafe impl bytemuck::Pod for Whirlpool {}

#[cfg(feature = "bytemuck")]
unsafe impl bytemuck::Zeroable for Whirlpool {}

#[cfg(feature = "bytemuck")]
impl BorshSerialize for Whirlpool {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(bytemuck::bytes_of(self))
    }
}

#[cfg(feature = "bytemuck")]
impl BorshDeserialize for Whirlpool {
    fn deserialize(buf: &mut &[u8]) -> std::io::Result<Self> {
        // Zero-copy deserialization using bytemuck
        bytemuck::try_from_bytes(data).map(|x| *x)
    }
}
```

### Modified Files

1. `rust-sdk/client/src/generated/accounts/whirlpool.rs`
2. `rust-sdk/client/src/generated/accounts/fixed_tick_array.rs`
3. `rust-sdk/client/src/generated/types/whirlpool_reward_info.rs`
4. `rust-sdk/client/src/generated/types/tick.rs`
5. `rust-sdk/client/Cargo.toml`

## Usage

### Without bytemuck (default, standard Borsh deserialization)

```toml
[dependencies]
orca_whirlpools_client = "2.0.2"
```

```rust
use orca_whirlpools_client::generated::accounts::Whirlpool;
use borsh::BorshDeserialize;

// Standard Borsh deserialization
let whirlpool = Whirlpool::deserialize(&mut &account_data[..])?;
```

### With bytemuck (opt-in, for zero-copy from on-chain data)

```toml
[dependencies]
orca_whirlpools_client = { version = "2.0.2", features = ["bytemuck"] }
```

```rust
use orca_whirlpools_client::generated::accounts::Whirlpool;

// Zero-copy deserialization from on-chain account data (packed layout)
let whirlpool = bytemuck::try_from_bytes::<Whirlpool>(&account_data[..Whirlpool::LEN])?;
```

## Benefits

### Without `bytemuck` feature (default, normal layout):
- ✅ **Fast field access** - aligned memory reads
- ✅ **Standard Borsh** - familiar serialization interface
- ✅ **Safe default** - standard Rust behavior
- ⚠️ **Cannot use** for zero-copy from on-chain data (different layout)

### With `bytemuck` feature (opt-in, packed layout):
- ✅ **Correct deserialization** of on-chain account data (packed)
- ✅ **Zero-copy** - no allocation, just pointer casting
- ✅ **Fast** - minimal overhead for deserialization
- ⚠️ **Slower field access** - unaligned memory reads (compiler generates safe code)

## Testing

The implementation has been tested with:
- ✅ Compilation without `bytemuck` feature (default)
- ✅ Compilation with `bytemuck` feature enabled
- ✅ Both configurations produce valid code

## Migration Guide

### For existing users
The default behavior uses standard Borsh deserialization with `#[repr(C)]`. This is the safe, standard approach.

### For users needing zero-copy from on-chain data
Add the `bytemuck` feature explicitly:
```toml
orca_whirlpools_client = { version = "2.0.2", features = ["bytemuck"] }
```

Or with git dependencies:
```toml
orca_whirlpools_client = { 
    git = "...", 
    rev = "...", 
    features = ["bytemuck"] 
}
```

## Future Considerations

This implementation provides flexibility for different use cases:
- **Standard usage**: Default behavior with normal Borsh deserialization
- **On-chain programs**: Opt-in to `bytemuck` feature for efficient zero-copy account parsing
- **Performance-critical**: Use `bytemuck` when you need minimal deserialization overhead
- **Testing**: Standard default makes testing easier with normal struct initialization

## Technical Notes

### Why packed is needed for on-chain data

The Anchor framework on Solana stores account data without alignment padding. When deserializing:
- With `#[repr(C)]`: Rust expects padding (e.g., `u128` at 16-byte boundary)
- With `#[repr(C, packed)]`: No padding, matches on-chain layout exactly

Using the wrong representation causes field offsets to mismatch, resulting in incorrect deserialized values.

### Safety considerations

The `unsafe impl bytemuck::Pod` is safe because:
1. All fields are `Pod` types (primitives, arrays of primitives, or other `Pod` types)
2. The struct is `#[repr(C, packed)]` with no padding
3. All bit patterns are valid for the types used

## References

- Original commit (adding bytemuck traits): `5865036c43056c0851be8de37675703597b8deec`
- Packed layout fix: `0d284940c10b90340ada6e7060e71e41eaed0ad8`
- Feature-gated implementation: (current commit)
