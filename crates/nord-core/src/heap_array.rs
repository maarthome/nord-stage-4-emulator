//! Zeroed fixed-size arrays built directly on the heap (TD-05).
//!
//! `Box::new([[0.0; N]; M])` first builds the whole array on the stack, and a debug build keeps
//! several copies of it (the value, the argument, the moved result), so one 128 KB reverb bank
//! cost more than 500 KB of stack. These helpers allocate the storage first and fill it in
//! place; the largest stack value is one row. Used by the constructors only, never in the audio
//! path (CLAUDE.md: no allocation in `process_block`).

/// A heap array of `N` zeros.
pub fn zeroed_array<const N: usize>() -> Box<[f32; N]> {
    match vec![0.0f32; N].into_boxed_slice().try_into() {
        Ok(b) => b,
        Err(_) => unreachable!("the vector has N elements"),
    }
}

/// A heap array of `ROWS` rows of `COLS` zeros; only one row is ever on the stack.
pub fn zeroed_rows<const ROWS: usize, const COLS: usize>() -> Box<[[f32; COLS]; ROWS]> {
    let rows: Vec<[f32; COLS]> = (0..ROWS).map(|_| [0.0; COLS]).collect();
    match rows.into_boxed_slice().try_into() {
        Ok(b) => b,
        Err(_) => unreachable!("the vector has ROWS rows"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeroed_array_has_the_requested_length_and_is_zero() {
        let a = zeroed_array::<1024>();
        assert_eq!(a.len(), 1024);
        assert!(a.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn zeroed_rows_matches_the_nested_array_it_replaces() {
        let a = zeroed_rows::<4, 8192>();
        let reference = [[0.0f32; 8192]; 4];
        assert_eq!(*a, reference);
    }

    #[test]
    fn large_arrays_do_not_touch_the_stack() {
        // 256 * 4096 floats = 4 MiB, far above a 256 KiB thread stack.
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| zeroed_rows::<256, 4096>().len())
            .unwrap();
        assert_eq!(handle.join().unwrap(), 256);
    }
}
