//! Creation of discriminants.
//!
//! The [`pot`] tool does not accept a discriminant as a command-line argument.
//! Instead, it [generates][1] the discriminant from a (much smaller) seed.
//! This file implements this process.  The table of precomputed constants used
//! is generated by `build.rs`.
//!
//! [1]: <https://github.com/Chia-Network/vdf-competition/blob/003b0d202d3b27058159f7a3f6a838e312e7d79e/inkfish/create_discriminant.py>
//! [`pot`]: <https://github.com/Chia-Network/vdf-competition/blob/003b0d202d3b27058159f7a3f6a838e312e7d79e/inkfish/cmds.py>

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

use super::BigNumExt;
use blake2::{digest::FixedOutput, Digest};
use core::default::Default;
use num_traits::Zero;
use std::u16;

fn random_bytes_from_seed<D>(seed: &[u8], byte_count: usize) -> Vec<u8>
where
    D: Digest + Default + FixedOutput,
{
    assert!(byte_count <= 32 * ((1 << 16) - 1));
    let mut blob = Vec::with_capacity(byte_count);
    let mut extra: u16 = 0;
    while blob.len() < byte_count {
        let mut hasher: D = D::default();
        hasher.input(seed);
        let extra_bits: [u8; 2] = [((extra & 0xFF00) >> 8) as _, (extra & 0xFF) as _];
        hasher.input(&extra_bits);
        blob.extend_from_slice(&hasher.fixed_result()[..]);
        extra += 1;
    }
    blob.resize(byte_count, 0);
    blob
}

/// Create a discriminant from a seed (a byte string) and a bit length (a
/// `u16`).  The discriminant is guaranteed to be a negative prime number that
/// fits in `length` bits, except with negligible probability (less than
/// 2^(-100)).  It is also guaranteed to equal 7 modulo 8.
///
/// This function uses blake2b to expand the seed.  Therefore, different seeds
/// will result in completely different discriminants with overwhelming
/// probability, unless `length` is very small.  However, this function is
/// deterministic: if it is called twice with identical seeds and lengths, it
/// will always return the same discriminant.
///
/// This function is guaranteed not to panic for any inputs whatsoever, unless
/// memory allocation fails and the allocator in use panics in that case.
pub fn create_discriminant<D, T>(seed: &[u8], length: u16) -> T
where
    D: Digest + Default + FixedOutput,
    T: BigNumExt,
{
    let (mut n, residue) = {
        // The number of “extra” bits (that don’t evenly fit in a byte)
        let extra: u8 = (length as u8) & 7;

        // The number of random bytes needed (the number of bytes that hold `length`
        // bits, plus 2).
        let random_bytes_len = ((usize::from(length) + 7) >> 3) + 2;
        let random_bytes = random_bytes_from_seed::<D>(seed, random_bytes_len);
        let (n, last_2) = random_bytes.split_at(random_bytes_len - 2);
        let numerator = (usize::from(last_2[0]) << 8) + usize::from(last_2[1]);

        // If there are any extra bits, right shift `n` so that it fits
        // in `length` bits, discarding the least significant bits.
        let n = T::from(n) >> usize::from((8 - extra) & 7);
        (n, RESIDUES[numerator % RESIDUES.len()])
    };
    n.setbit(usize::from(length - 1));
    debug_assert!(n >= Zero::zero());
    let rem = n.frem_u32(M);

    // HACK HACK `rust-gmp` doesn’t expose += and -= with i32 or i64
    if residue > rem {
        n = n + u64::from(residue - rem);
    } else {
        n = n - u64::from(rem - residue);
    }
    debug_assert!(n >= Zero::zero());

    // This generates the smallest prime ≥ n that is of the form n + m*x.
    loop {
        // Speed up prime-finding by quickly ruling out numbers
        // that are known to be composite.
        let mut sieve = vec![false; 1 << 16];
        for &(p, q) in SIEVE_INFO.iter() {
            // The reference implementation changes the sign of `n` before taking its
            // remainder. Instead, we leave `n` as positive, but use ceiling
            // division instead of floor division.  This is mathematically
            // equivalent and potentially faster.
            let mut i: usize = (n.crem_u16(p) as usize * q as usize) % p as usize;
            while i < sieve.len() {
                sieve[i] = true;
                i += p as usize;
            }
        }

        for (i, &x) in sieve.iter().enumerate() {
            let i = i as u32;
            if !x {
                let q = u64::from(M) * u64::from(i);
                n = n + q;
                if n.probab_prime(2) {
                    return -n;
                }
                n = n - q;
            }
        }

        // M is set to a number with many prime factors so the results are
        // more uniform https://eprint.iacr.org/2011/401.pdf
        n = n + (u64::from(M) * (1 << 16)) as u64
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{gmp_classgroup::GmpClassGroup, ClassGroup};
    type Mpz = <GmpClassGroup as ClassGroup>::BigNum;
    use sha2::Sha256;
    use std::str::FromStr;

    #[test]
    fn check_discriminant_1() {
        assert_eq!(
            create_discriminant::<Sha256, Mpz>(b"\xaa", 40),
            (-685_537_176_559i64).into()
        );
    }

    #[test]
    fn check_discriminant_3() {
        assert_eq!(
            create_discriminant::<Sha256, Mpz>(b"\xaa", 1024),
            Mpz::from_str(
                "-112084717443890964296630631725167420667316836131914185144761\
                 7438378168250988242739496385274308134767869324152361453294226\
                 8295868231081182819214054220080323345750407342623884342617809\
                 8794592117225058677336074005099949757067786815439982423354682\
                 0386024058617141397148586038290164093146862666602485017735298\
                 03183"
            )
            .unwrap()
        )
    }

    #[test]
    fn check_discriminant_2() {
        assert_eq!(
            create_discriminant::<Sha256, Mpz>(b"\xaa", 2048),
            -Mpz::from_str(
                "201493927071865251625903550712920535753645598483515670853547009\
                 878440933309489362800393797428711071833308081461824159206915864\
                 150805748296170245037221957772328044276705571745811271212292422\
                 075849739248257870371300001313586036515879618764093772248760562\
                 386804073478433157526816295216137723803793411828867470089409596\
                 238958950007370719325959579892866588928887249912429688364409867\
                 895510817680171869190054122881274299350947669820596157115994418\
                 034091728887584373727555384075665624624856766441009974642693066\
                 751400054217209981490667208950669417773785631693879782993019167\
                 69407006303085854796535778826115224633447713584423"
            )
            .unwrap()
        );
    }
    #[test]
    fn check_random_bytes() {
        assert_eq!(
            &random_bytes_from_seed::<Sha256>(b"\xaa", 7),
            b"\x9f\x9d*\xe5\xe7<\xcb"
        );
        assert_eq!(
            &random_bytes_from_seed::<Sha256>(b"\xaa", 258)[..],
            &b"\x9f\x9d*\xe5\xe7<\xcbq\xa4q\x8e\
                   \xbc\xf0\xe3:\xa2\x98\xf8\xbd\xdc\xaa\xcbi\xcb\x10\xff\x0e\xafv\xdb\xec!\xc4K\
                   \xc6Jf\xf3\xa5\xda.7\xb7\xef\x87I\x85\xb8YX\xfc\xf2\x03\xa1\x8f4\xaf`\xab\xae]n\
                   \xcc,g1\x12EI\xc7\xd5\xe2\xfc\x8b\x9a\xde\xd5\xf3\x8f'\xcd\x08\x0fU\xc7\xee\xa85\
                   [>\x87]\x07\x82\x00\x13\xce\xf7\xc3/@\xef\x08v\x8f\x85\x87dm(1\x8b\xd9w\xffA]xzY\
                   \xa0,\xebz\xff\x03$`\x91\xb66\x88-_\xa9\xf1\xc5\x8e,\x15\xae\x8f\x04\rvhnU3f\x84\
                   [{$\xa6l\x95w\xa9\x1f\xba\xa8)\x05\xe6\x8f\x167o\x11/X\x9cl\xab\x9c\xcb}\xec\x88\
                   \xf8\xa5\xabXpY\xb0\x88\xed@r\x05\xba\\\x03\xf6\x91\xf8\x03\xca\x18\x1c\xcdH\x1c\
                   \x91\xe1V\xed;\x94oJ\xa8 \xa4\x97\xb7K\xce\xc4e\xea\xa2\xbf\x8b\x1f\x90\x87\xc8\
                   \x15\xee\x0e\x0fPC:\xb5\xe1g\x97\xea/_\x86c\xaf\x12Wp\xfd\x11\xdb\x17\xe6\x9f\
                   \xa5\x8a"[..]
        );
    }
}
