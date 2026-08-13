# opt-simd-portable

> Add explicit SIMD only after representative benchmarks beat optimized scalar code

## Why It Matters

SIMD processes multiple values per instruction, but speedup depends on the
algorithm, data shape, target CPU, compiler, and surrounding memory traffic.
Stable Rust can rely on LLVM autovectorization or maintained target-aware
crates, while `std::simd` remains nightly. Keep optimized scalar code as the
baseline and retain explicit SIMD only when representative benchmarks win on
every supported deployment class.

## Bad

Assume a lane count or intrinsic is faster, benchmark only one development
machine, or remove the scalar fallback.

## Good

```rust
fn add_arrays(a: &[f32], b: &[f32], out: &mut [f32]) {
    assert_eq!(a.len(), b.len());
    assert_eq!(a.len(), out.len());
    for ((x, y), o) in a.iter().zip(b).zip(out.iter_mut()) {
        *o = x + y;
    }
}

// Verify optimized assembly or benchmarks rather than assuming vectorization.
// chunks_exact gives fixed-width chunks, not an alignment guarantee.
```

## Portable SIMD (Nightly)

```rust
#![feature(portable_simd)]
use std::simd::{f32x8, prelude::*};

fn sum_simd(data: &[f32]) -> f32 {
    let (prefix, middle, suffix) = data.as_simd::<8>();

    // Handle unaligned prefix
    let mut sum = prefix.iter().sum::<f32>();

    // SIMD loop - 8 floats at a time
    let mut simd_sum = f32x8::splat(0.0);
    for chunk in middle {
        simd_sum += *chunk;
    }
    sum += simd_sum.reduce_sum();

    // Handle unaligned suffix
    sum += suffix.iter().sum::<f32>();

    sum
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    let (a_pre, a_mid, a_suf) = a.as_simd::<8>();
    let prefix_len = a_pre.len();
    let middle_len = a_mid.len() * 8;
    let (b_pre, b_rest) = b.split_at(prefix_len);
    let (b_mid, b_suf) = b_rest.split_at(middle_len);

    let scalar: f32 = a_pre.iter().zip(b_pre).map(|(x, y)| x * y).sum();

    let mut simd_sum = f32x8::splat(0.0);
    for (av, bv) in a_mid.iter().zip(b_mid.chunks_exact(8)) {
        simd_sum += *av * f32x8::from_slice(bv);
    }

    let suffix: f32 = a_suf.iter().zip(b_suf).map(|(x, y)| x * y).sum();

    scalar + simd_sum.reduce_sum() + suffix
}
```

The second input is segmented by the first input's indices, not by its own
alignment. Independent `as_simd` splits can have different prefix lengths even
when the slices have equal lengths. Floating-point SIMD reductions may also
round differently from the scalar baseline, so define and test a numerical
tolerance for the product contract.

## wide Crate (Stable)

```rust
use wide::*;

fn process_simd(data: &mut [f32]) {
    let mut chunks = data.chunks_exact_mut(8);
    for chunk in &mut chunks {
        let v = f32x8::from(&*chunk);
        let result = v * f32x8::splat(2.0) + f32x8::splat(1.0);
        chunk.copy_from_slice(&result.to_array());
    }

    for value in chunks.into_remainder() {
        *value = *value * 2.0 + 1.0;
    }
}
```

## Platform-Specific (When Needed)

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

fn sum_scalar(data: &[f32]) -> f32 {
    data.iter().sum()
}

fn sum_dispatch(data: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    if std::is_x86_feature_detected!("avx2") {
        // SAFETY: runtime feature detection established AVX2 support.
        return unsafe { sum_avx2(data) };
    }
    sum_scalar(data)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
/// # Safety
///
/// The caller must establish AVX2 support, for example with
/// `is_x86_feature_detected!("avx2")`, before calling this function.
unsafe fn sum_avx2(data: &[f32]) -> f32 {
    let mut acc = _mm256_setzero_ps();
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        // SAFETY: chunks_exact(8) yields eight initialized f32 values and the
        // unaligned load accepts any valid f32 alignment.
        let v = unsafe { _mm256_loadu_ps(chunk.as_ptr()) };
        acc = _mm256_add_ps(acc, v);
    }

    // store the 8 lanes, then finish the reduction (and the remainder) in scalar
    let mut lanes = [0.0f32; 8];
    // SAFETY: lanes provides space for eight initialized f32 outputs and the
    // unaligned store accepts its address.
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc) };
    lanes.iter().sum::<f32>() + chunks.remainder().iter().sum::<f32>()
}
```

## Choosing an Approach

| Approach | Stability | Portability | Control |
|----------|-----------|-------------|---------|
| Autovectorization | Stable | Excellent | Low |
| `wide` crate | Stable | Good | Medium |
| Portable SIMD | Nightly | Excellent | High |
| Intrinsics | Stable per supported architecture | Architecture-specific | Maximum |

Compile-time `target_feature` is not runtime dispatch. A portable binary needs
a scalar baseline plus a checked dispatch path, and tests on hardware or an
emulator that actually executes every advertised path. For floating-point
algorithms, compare every path to the scalar contract using a documented
tolerance rather than requiring bit-identical reduction order.

## See Also

- [opt-target-cpu](opt-target-cpu.md) - enable SIMD features
- [opt-bounds-check](opt-bounds-check.md) - verify safe hot loops before unchecked access
- [perf-profile-first](perf-profile-first.md) - identify vectorization opportunities
