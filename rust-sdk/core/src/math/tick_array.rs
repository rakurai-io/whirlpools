use crate::{
    CoreError, TickArrayFacade, TickFacade, INVALID_TICK_ARRAY_SEQUENCE, INVALID_TICK_INDEX,
    MAX_TICK_INDEX, MIN_TICK_INDEX, TICK_ARRAY_NOT_EVENLY_SPACED, TICK_ARRAY_SIZE,
    TICK_INDEX_OUT_OF_BOUNDS, TICK_SEQUENCE_EMPTY,
};

pub use solana_program::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};

use super::{
    get_initializable_tick_index, get_next_initializable_tick_index,
    get_prev_initializable_tick_index, get_tick_array_start_tick_index,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickArraySequence {
    tick_arrays: HashMap<Pubkey, TickArrayFacade>,
    current_tick_arrays: HashSet<i32>,
    tick_spacing: u16,
}

impl TickArraySequence {
    pub fn new(
        tick_arrays: impl IntoIterator<Item = (Pubkey, TickArrayFacade)>,
        tick_spacing: u16,
    ) -> Result<Self, CoreError> {
        let mut map = HashMap::new();
        let mut start_indices = Vec::new();
        
        for (pubkey, facade) in tick_arrays {
            start_indices.push(facade.start_tick_index);
            map.insert(pubkey, facade);
        }
        
        if map.is_empty() {
            return Err(TICK_SEQUENCE_EMPTY);
        }
        
        start_indices.sort_unstable();
        validate_evenly_spaced(&start_indices, tick_spacing)?;
        
        let current_tick_arrays: HashSet<i32> = start_indices.into_iter().collect();
        
        Ok(Self {
            tick_arrays: map,
            current_tick_arrays,
            tick_spacing,
        })
    }

    /// Same validation as [`Self::new`], but generates unique pubkeys based on start_tick_index.
    pub fn with_default_pubkeys(
        tick_arrays: Vec<Option<TickArrayFacade>>,
        tick_spacing: u16,
    ) -> Result<Self, CoreError> {
        let tick_arrays_iter = tick_arrays
            .into_iter()
            .flatten()
            .map(|f| {
                // Generate unique pubkey based on start_tick_index to avoid HashMap collisions
                let mut bytes = [0u8; 32];
                bytes[0..4].copy_from_slice(&f.start_tick_index.to_le_bytes());
                (Pubkey::new_from_array(bytes), f)
            });
        Self::new(tick_arrays_iter, tick_spacing)
    }

    /// Insert or replace tick arrays by `start_tick_index`, then sort and validate spacing (same rules as `new`).
    pub fn add_new_tick_arrays(
        &mut self,
        new_arrays: impl IntoIterator<Item = (Pubkey, TickArrayFacade)>,
    ) -> Result<(), CoreError> {
        for (pubkey, facade) in new_arrays {
            let start_tick_index = facade.start_tick_index;
            
            // Remove old entry with same start_tick_index if it exists
            self.tick_arrays.retain(|_, v| v.start_tick_index != start_tick_index);
            
            // Insert new entry
            self.tick_arrays.insert(pubkey, facade);
            self.current_tick_arrays.insert(start_tick_index);
        }
        
        // Validate spacing
        let mut start_indices: Vec<i32> = self.current_tick_arrays.iter().copied().collect();
        start_indices.sort_unstable();
        validate_evenly_spaced(&start_indices, self.tick_spacing)?;
        
        Ok(())
    }

    /// Returns the first valid tick index in the sequence.
    pub fn start_index(&self) -> i32 {
        self.current_tick_arrays
            .iter()
            .min()
            .copied()
            .unwrap_or(0)
            .max(MIN_TICK_INDEX)
    }

    /// Returns the last valid tick index in the sequence.
    pub fn end_index(&self) -> i32 {
        let last_start_index = self.current_tick_arrays
            .iter()
            .max()
            .copied()
            .unwrap_or(0);
        let end_index = last_start_index + TICK_ARRAY_SIZE as i32 * self.tick_spacing as i32 - 1;
        end_index.min(MAX_TICK_INDEX)
    }

    fn get_tick_array_for_tick(&self, tick_index: i32) -> Option<&TickArrayFacade> {
        let tick_array_start_index = get_tick_array_start_tick_index(tick_index, self.tick_spacing);
        
        if !self.current_tick_arrays.contains(&tick_array_start_index) {
            return None;
        }
        
        self.tick_arrays
            .values()
            .find(|ta| ta.start_tick_index == tick_array_start_index)
    }

    pub fn tick(&self, tick_index: i32) -> Result<&TickFacade, CoreError> {
        if (tick_index < self.start_index()) || (tick_index > self.end_index()) {
            return Err(TICK_INDEX_OUT_OF_BOUNDS);
        }
        if (tick_index % self.tick_spacing as i32) != 0 {
            return Err(INVALID_TICK_INDEX);
        }
        
        let tick_array = self.get_tick_array_for_tick(tick_index)
            .ok_or(INVALID_TICK_ARRAY_SEQUENCE)?;
        
        let index_in_array = (tick_index - tick_array.start_tick_index) / self.tick_spacing as i32;
        Ok(&tick_array.ticks[index_in_array as usize])
    }

    pub fn next_initialized_tick(
        &self,
        tick_index: i32,
    ) -> Result<(Option<&TickFacade>, i32), CoreError> {
        let array_end_index = self.end_index();
        if tick_index >= array_end_index {
            return Err(INVALID_TICK_ARRAY_SEQUENCE);
        }
        let mut next_index = tick_index;
        loop {
            next_index = get_next_initializable_tick_index(next_index, self.tick_spacing);
            // If at the end of the sequence, we don't have tick info but can still return the next tick index
            if next_index > array_end_index {
                return Ok((None, array_end_index));
            }
            let tick = self.tick(next_index)?;
            if tick.initialized {
                return Ok((Some(tick), next_index));
            }
        }
    }

    pub fn prev_initialized_tick(
        &self,
        tick_index: i32,
    ) -> Result<(Option<&TickFacade>, i32), CoreError> {
        let array_start_index = self.start_index();
        if tick_index < array_start_index {
            return Err(INVALID_TICK_ARRAY_SEQUENCE);
        }
        let mut prev_index =
            get_initializable_tick_index(tick_index, self.tick_spacing, Some(false));
        loop {
            // If at the start of the sequence, we don't have tick info but can still return the previous tick index
            if prev_index < array_start_index {
                return Ok((None, array_start_index));
            }
            let tick = self.tick(prev_index)?;
            if tick.initialized {
                return Ok((Some(tick), prev_index));
            }
            prev_index = get_prev_initializable_tick_index(prev_index, self.tick_spacing);
        }
    }

    /// Returns a reference to the tick array associated with the given pubkey.
    pub fn get_tick_array(&self, pubkey: &Pubkey) -> Option<&TickArrayFacade> {
        self.tick_arrays.get(pubkey)
    }

    /// Returns an iterator over all (Pubkey, TickArrayFacade) pairs in the sequence.
    pub fn tick_arrays_iter(&self) -> impl Iterator<Item = (&Pubkey, &TickArrayFacade)> {
        self.tick_arrays.iter()
    }

    /// Checks if a tick array with the given start index exists in the sequence.
    pub fn contains_tick_array_at(&self, start_tick_index: i32) -> bool {
        self.current_tick_arrays.contains(&start_tick_index)
    }

    /// Returns an iterator over all start tick indices in the sequence.
    pub fn start_tick_indices(&self) -> impl Iterator<Item = &i32> {
        self.current_tick_arrays.iter()
    }

    /// Returns an iterator over all tick array pubkeys in the sequence.
    pub fn tick_array_pubkeys(&self) -> impl Iterator<Item = &Pubkey> {
        self.tick_arrays.keys()
    }

    /// Returns a reference to the internal HashSet of current tick array start indices.
    /// This is useful for checking which tick arrays are currently loaded.
    pub fn current_tick_arrays(&self) -> &HashSet<i32> {
        &self.current_tick_arrays
    }

    /// Returns a reference to the internal HashMap of tick arrays.
    /// This is useful when you need direct access to the tick arrays by pubkey.
    pub fn tick_arrays(&self) -> &HashMap<Pubkey, TickArrayFacade> {
        &self.tick_arrays
    }
}

// internal functions

fn validate_evenly_spaced(
    start_indices: &[i32],
    tick_spacing: u16,
) -> Result<(), CoreError> {
    if start_indices.is_empty() {
        return Err(TICK_SEQUENCE_EMPTY);
    }

    let required_tick_array_spacing = TICK_ARRAY_SIZE as i32 * tick_spacing as i32;
    for i in 0..start_indices.len() - 1 {
        let current = start_indices[i];
        let next = start_indices[i + 1];
        let actual_spacing = next - current;
        if actual_spacing != required_tick_array_spacing {
            return Err(TICK_ARRAY_NOT_EVENLY_SPACED);
        }
    }
    Ok(())
}

#[cfg(all(test, not(feature = "wasm")))]
mod tests {
    use super::*;
    use crate::get_tick_array_start_tick_index;

    fn test_pubkey(i: u8) -> Pubkey {
        let mut b = [0u8; 32];
        b[0] = i;
        Pubkey::new_from_array(b)
    }

    fn test_tick(initialized: bool, liquidity_net: i128) -> TickFacade {
        TickFacade {
            initialized,
            liquidity_net,
            ..TickFacade::default()
        }
    }

    fn test_ticks_initialized() -> [TickFacade; TICK_ARRAY_SIZE] {
        (0..TICK_ARRAY_SIZE)
            .map(|x| test_tick(true, x as i128))
            .collect::<Vec<TickFacade>>()
            .try_into()
            .unwrap()
    }

    fn test_ticks_uninitialized() -> [TickFacade; TICK_ARRAY_SIZE] {
        (0..TICK_ARRAY_SIZE)
            .map(|_| test_tick(false, 0))
            .collect::<Vec<TickFacade>>()
            .try_into()
            .unwrap()
    }

    fn test_ticks_alternating_initialized() -> [TickFacade; TICK_ARRAY_SIZE] {
        (0..TICK_ARRAY_SIZE)
            .map(|x| {
                let initialized = x & 1 == 1;
                test_tick(initialized, if initialized { x as i128 } else { 0 })
            })
            .collect::<Vec<TickFacade>>()
            .try_into()
            .unwrap()
    }

    fn test_sequence_with_one_tick_array(
        tick_spacing: u16,
        ticks: [TickFacade; TICK_ARRAY_SIZE],
        start_tick_index: i32,
    ) -> TickArraySequence {
        let one = TickArrayFacade {
            start_tick_index,
            ticks,
        };
        TickArraySequence::new(
            vec![(test_pubkey(1), one)],
            tick_spacing,
        )
        .unwrap()
    }

    fn test_sequence(
        tick_spacing: u16,
        ticks: [TickFacade; TICK_ARRAY_SIZE],
    ) -> TickArraySequence {
        let one = TickArrayFacade {
            start_tick_index: -(TICK_ARRAY_SIZE as i32 * tick_spacing as i32),
            ticks,
        };
        let two = TickArrayFacade {
            start_tick_index: 0,
            ticks,
        };
        let three = TickArrayFacade {
            start_tick_index: TICK_ARRAY_SIZE as i32 * tick_spacing as i32,
            ticks,
        };
        TickArraySequence::new(
            vec![
                (test_pubkey(1), one),
                (test_pubkey(2), two),
                (test_pubkey(3), three),
            ],
            tick_spacing,
        )
        .unwrap()
    }

    #[test]
    fn test_tick_array_start_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        assert_eq!(sequence.start_index(), -1408);
    }

    #[test]
    fn test_tick_array_end_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        assert_eq!(sequence.end_index(), 2815);
    }

    #[test]
    fn test_get_tick() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        assert_eq!(sequence.tick(-1408).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(-16).map(|x| x.liquidity_net), Ok(87));
        assert_eq!(sequence.tick(0).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(16).map(|x| x.liquidity_net), Ok(1));
        assert_eq!(sequence.tick(1408).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(1424).map(|x| x.liquidity_net), Ok(1));
    }

    #[test]
    fn test_get_tick_large_tick_spacing() {
        let sequence: TickArraySequence =
            test_sequence(32896, test_ticks_alternating_initialized());
        assert_eq!(sequence.tick(-427648).map(|x| x.liquidity_net), Ok(75));
        assert_eq!(sequence.tick(0).map(|x| x.liquidity_net), Ok(0));
        assert_eq!(sequence.tick(427648).map(|x| x.liquidity_net), Ok(13));
    }

    #[test]
    fn test_get_tick_errors() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());

        let out_out_bounds_lower = sequence.tick(-1409);
        assert!(matches!(
            out_out_bounds_lower,
            Err(TICK_INDEX_OUT_OF_BOUNDS)
        ));

        let out_of_bounds_upper = sequence.tick(2817);
        assert!(matches!(out_of_bounds_upper, Err(TICK_INDEX_OUT_OF_BOUNDS)));

        let invalid_tick_index = sequence.tick(1);
        assert!(matches!(invalid_tick_index, Err(INVALID_TICK_INDEX)));

        let invalid_negative_tick_index = sequence.tick(-1);
        assert!(matches!(
            invalid_negative_tick_index,
            Err(INVALID_TICK_INDEX)
        ));
    }

    #[test]
    fn test_get_next_initializable_tick_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(0);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_next_initializable_tick_index_off_spacing() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(-17);
        assert_eq!(pair.map(|x| x.1), Ok(-16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(87)));
    }

    #[test]
    fn test_get_next_initializable_tick_cross_array() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(1392);
        assert_eq!(pair.map(|x| x.1), Ok(1424));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_next_initializable_tick_skip_uninitialized() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.next_initialized_tick(-1);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_next_initializable_tick_INVALID_TICK_ARRAY_SEQUENCE() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair_2813 = sequence.next_initialized_tick(2813);
        let pair_2814 = sequence.next_initialized_tick(2814);
        let pair_2815 = sequence.next_initialized_tick(2815);
        let pair_2816 = sequence.next_initialized_tick(2816);
        assert_eq!(pair_2813, Ok((None, 2815)));
        assert_eq!(pair_2814, Ok((None, 2815)));
        assert_eq!(pair_2815, Err(INVALID_TICK_ARRAY_SEQUENCE));
        assert_eq!(pair_2816, Err(INVALID_TICK_ARRAY_SEQUENCE));
    }

    #[test]
    fn test_get_next_initializable_tick_with_last_initializable_tick_initialized() {
        let sequence = test_sequence(16, test_ticks_initialized());
        let pair_2799 = sequence.next_initialized_tick(2799);
        let pair_2800 = sequence.next_initialized_tick(2800);
        assert_eq!(pair_2799, Ok((Some(&test_tick(true, 87)), 2800)));
        assert_eq!(pair_2800, Ok((None, 2815)));
    }

    #[test]
    fn test_get_next_initializable_tick_with_last_initializable_tick_uninitialized() {
        let sequence = test_sequence(16, test_ticks_uninitialized());
        let pair_2799 = sequence.next_initialized_tick(2799);
        assert_eq!(pair_2799, Ok((None, 2815)));
    }

    #[test]
    fn test_get_next_initializable_tick_in_end_tick_array_with_uninitialized_ticks_ts_16() {
        let tick_spacing = 16;
        let start_tick_index = get_tick_array_start_tick_index(MAX_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.next_initialized_tick(start_tick_index);
        assert_eq!(pair, Ok((None, MAX_TICK_INDEX)));
    }

    #[test]
    fn test_get_next_initializable_tick_in_end_tick_array_with_uninitialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MAX_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.next_initialized_tick(start_tick_index);
        assert_eq!(pair, Ok((None, MAX_TICK_INDEX)));
    }

    #[test]
    fn test_get_next_initializable_tick_in_end_tick_array_with_initialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MAX_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_initialized(),
            start_tick_index,
        );
        let pair = sequence.next_initialized_tick(MAX_TICK_INDEX - 1);
        assert_eq!(pair, Ok((Some(&test_tick(true, 28)), MAX_TICK_INDEX)));
    }

    #[test]
    fn test_get_prev_initializable_tick_index() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(32);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_prev_initializable_tick_index_off_spacing() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(-1);
        assert_eq!(pair.map(|x| x.1), Ok(-16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(87)));
    }

    #[test]
    fn test_get_prev_initializable_tick_skip_uninitialized() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(33);
        assert_eq!(pair.map(|x| x.1), Ok(16));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(1)));
    }

    #[test]
    fn test_get_prev_initializable_tick_cross_array() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair = sequence.prev_initialized_tick(1408);
        assert_eq!(pair.map(|x| x.1), Ok(1392));
        assert_eq!(pair.map(|x| x.0.map(|x| x.liquidity_net)), Ok(Some(87)));
    }

    #[test]
    fn test_get_prev_initialized_tick_INVALID_TICK_ARRAY_SEQUENCE() {
        let sequence = test_sequence(16, test_ticks_alternating_initialized());
        let pair_1407 = sequence.prev_initialized_tick(-1407);
        let pair_1408 = sequence.prev_initialized_tick(-1408);
        let pair_1409 = sequence.prev_initialized_tick(-1409);
        let pair_1410 = sequence.prev_initialized_tick(-1410);
        assert!(matches!(pair_1407, Ok((None, -1408))));
        assert!(matches!(pair_1408, Ok((None, -1408))));
        assert!(matches!(pair_1409, Err(INVALID_TICK_ARRAY_SEQUENCE)));
        assert!(matches!(pair_1410, Err(INVALID_TICK_ARRAY_SEQUENCE)));
    }

    #[test]
    fn test_get_prev_initializable_tick_with_first_initializable_tick_initialized() {
        let sequence = test_sequence(16, test_ticks_initialized());
        let pair = sequence.prev_initialized_tick(-1408);
        assert_eq!(pair, Ok((Some(&test_tick(true, 0)), -1408)));
    }

    #[test]
    fn test_get_prev_initializable_tick_with_first_initializable_tick_uninitialized() {
        let sequence = test_sequence(16, test_ticks_uninitialized());
        let pair = sequence.prev_initialized_tick(-1408);
        assert_eq!(pair, Ok((None, -1408)));
    }

    #[test]
    fn test_get_prev_initializable_tick_in_first_tick_array_with_uninitialized_ticks_ts_16() {
        let tick_spacing = 16;
        let start_tick_index = get_tick_array_start_tick_index(MIN_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.prev_initialized_tick(MIN_TICK_INDEX + tick_spacing as i32);
        assert_eq!(pair, Ok((None, MIN_TICK_INDEX)));
    }

    #[test]
    fn test_get_prev_initializable_tick_in_first_tick_array_with_uninitialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MIN_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_uninitialized(),
            start_tick_index,
        );
        let pair = sequence.prev_initialized_tick(MIN_TICK_INDEX + tick_spacing as i32);
        assert_eq!(pair, Ok((None, MIN_TICK_INDEX)));
    }

    #[test]
    fn test_get_prev_initializable_tick_in_first_tick_array_with_initialized_ticks_ts_1() {
        let tick_spacing = 1;
        let start_tick_index = get_tick_array_start_tick_index(MIN_TICK_INDEX, tick_spacing);
        let sequence = test_sequence_with_one_tick_array(
            tick_spacing,
            test_ticks_initialized(),
            start_tick_index,
        );
        let pair = sequence.prev_initialized_tick(MIN_TICK_INDEX);
        assert_eq!(pair, Ok((Some(&test_tick(true, 60)), MIN_TICK_INDEX)));
    }
}
