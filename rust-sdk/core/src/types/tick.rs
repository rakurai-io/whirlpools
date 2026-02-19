#![allow(non_snake_case)]

#[cfg(feature = "wasm")]
use serde_big_array::BigArray;

#[cfg(feature = "wasm")]
use orca_whirlpools_macros::wasm_expose;

use crate::TICK_ARRAY_SIZE;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "wasm", wasm_expose)]
pub struct TickRange {
    pub tick_lower_index: i32,
    pub tick_upper_index: i32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "wasm", wasm_expose)]
#[repr(C)]
pub struct TickFacade {
    pub initialized: bool,
    pub liquidity_net: i128,
    pub liquidity_gross: u128,
    pub fee_growth_outside_a: u128,
    pub fee_growth_outside_b: u128,
    #[cfg_attr(feature = "wasm", tsify(type = "bigint[]"))]
    pub reward_growths_outside: [u128; 3],
}

unsafe impl bytemuck::Pod for TickFacade {}
unsafe impl bytemuck::Zeroable for TickFacade {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "wasm", wasm_expose)]
#[repr(C)]
pub struct TickArrayFacade {
    pub start_tick_index: i32,
    #[cfg_attr(feature = "wasm", serde(with = "BigArray"))]
    pub ticks: [TickFacade; TICK_ARRAY_SIZE],
}

unsafe impl bytemuck::Pod for TickArrayFacade {}
unsafe impl bytemuck::Zeroable for TickArrayFacade {}
