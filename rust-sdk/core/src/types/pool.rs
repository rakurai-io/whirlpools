#![allow(non_snake_case)]

use crate::NUM_REWARDS;

#[cfg(feature = "wasm")]
use orca_whirlpools_macros::wasm_expose;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "wasm", wasm_expose)]
#[repr(C)]
pub struct WhirlpoolFacade {
    pub tick_spacing: u16,
    pub fee_rate: u16,
    pub protocol_fee_rate: u16,
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub tick_current_index: i32,
    pub fee_growth_global_a: u128,
    pub fee_growth_global_b: u128,
    pub reward_last_updated_timestamp: u64,
    #[cfg_attr(feature = "wasm", tsify(type = "WhirlpoolRewardInfoFacade[]"))]
    pub reward_infos: [WhirlpoolRewardInfoFacade; NUM_REWARDS],
}

unsafe impl bytemuck::Pod for WhirlpoolFacade {}
unsafe impl bytemuck::Zeroable for WhirlpoolFacade {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "wasm", wasm_expose)]
#[repr(C)]
pub struct WhirlpoolRewardInfoFacade {
    pub emissions_per_second_x64: u128,
    pub growth_global_x64: u128,
}

unsafe impl bytemuck::Pod for WhirlpoolRewardInfoFacade {}
unsafe impl bytemuck::Zeroable for WhirlpoolRewardInfoFacade {}
