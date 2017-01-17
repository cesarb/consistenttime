//Copyright 2016 William Cody Laeder
//
//Licensed under the Apache License, Version 2.0 (the "License");
//you may not use this file except in compliance with the License.
//you may obtain a copy of the License at
//
//  http://www.apache.org/licenses/LICENSE-2.0
//
//Unless required by applicable law or agreed to in writing, software
//distrubuted under the License is distrubuted on as "AS IS" BASIS,
//WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//See the License for the specific language governing permissions and
//limitations under the License.


//!Consistent Time
//!
//!The goal of this crate is to offer constant time functions which most
//!cryptographic computing protocols require to prevent side channel 
//!timing attacks. 
//!
//!These algorithms are not implemented to be efficient. But to take the
//!same number of processor cycles if their outcome/path is true, or false.
//!The reference used for this crate is [Go-Lang's
//!crypto/subtile](https://golang.org/src/crypto/subtle/constant_time.go)
//!Which implements a handful of constant time algorithms.
//!
//!I took the liberity of generalizing them out to all unsigned sizes
//!supported by Rust-Lang. Everything inside of this crate is defined
//!as a macro. This makes writing the extremely repetive code for all
//!types a lot easier.
//!
//!There is internal unsafe code to handle converting `bool` to `u8`
//!and vice versa. The machine instructions generated for these
//!operations involve no branches or comparison operators,
//!see the notes in the source code.
//!
//!As of the most recent commit there has been an _extreme_ divergence
//!from the Go-Lang source. LLVM does MUCH heavier optimizations then 
//!Go-ASM does and some _combat_ was necessary. As of
//!
//!`consistenttime = "0.2"`
//!
//!I am reasonably confident it provides the advertised guarantees.

#![no_std]
use core::mem::transmute as trans;

macro_rules! max { ($t:ident) => { ::core::$t::MAX } }


/*
 * Rust booleans are effectively u8's with typing sugar.
 * True = 0x01
 * False = 0x00
 *
 * One can recover the true/false value via unsafe function
 *
 * fn to_val<X>(b: bool) -> X {
 *      let val: u8 = unsafe{ ::core::mem::transmute(b) };
 *      val as X
 * }
 *
 * For the type u64 (and using the sugar [#inline(never)]
 * this will compile down to:
 *
 *    mov rax dil
 *    ret
 *
 * One can likely from that example determine how _other_
 * integer formats are dervived.
 *
 * The test below asserts this behavior.
 *
 * :.:.:
 *
 * When converting from ~some~ uint to a bool the reverse
 * is sligtly true. Consider the equality operation
 *
 *    let mut z: u16 = MAX ^ (x^y);
 *    z >> 8;
 *    z >> 4;
 *    z >> 2;
 *    z >> 1;
 *    let val = z as u8;
 *    unsafe{ ::core::mem::transmute(val) }; //returns bool
 *
 * The ASM generated for the last two operations
 *
 *    let val = z as u8;
 *    unsafe{ ::core::mem::transmute(val)};
 *
 * It is simply
 *
 *    andl $1, %eax
 *    retq
 *
 * The typing is gone at compile time.
 */
#[test]
fn test_bool_representation() {
    let t: bool = true;
    let f: bool = false;
    let t_val: u8 = unsafe{ ::core::mem::transmute(t) };
    let f_val: u8 = unsafe{ ::core::mem::transmute(f) };
    assert_eq!( t_val, 0x01u8);
    assert_eq!( f_val, 0x00u8);
}

pub trait ConstantTime : Sized {
    fn ct_eq(x: Self, y: Self) -> bool;
    fn ct_eq_slice(x: &[Self], y: &[Self]) -> bool;
    fn ct_select(flag: bool, x: Self, y: Self) -> Self;
    fn ct_copy(flag: bool, x: &mut [Self], y: &[Self]);
}
pub fn ct_eq<T>(x: T, y: T) -> bool
  where T: ConstantTime {
    <T as ConstantTime>::ct_eq(x,y)
}
pub fn ct_eq_slice<T>(x: &[T], y: &[T]) -> bool
  where T: ConstantTime {
    <T as ConstantTime>::ct_eq_slice(x,y)
}
pub fn ct_select<T>(flag: bool, x: T, y: T) -> T
  where T: ConstantTime {
    <T as ConstantTime>::ct_select(flag,x,y)
}
pub fn ct_copy<T>(flag: bool, x: &mut [T], y: &[T])
  where T: ConstantTime {
    <T as ConstantTime>::ct_copy(flag,x,y);
}

macro_rules! impl_ConstantTime {
    ($code: ident, $eq: ident, $slice_eq: ident, $select: ident, $copy: ident) => {
        impl ConstantTime for $code {
            fn ct_eq( x: $code, y: $code) -> bool {
                $eq(x,y)
            }
            fn ct_eq_slice( x: &[$code], y: &[$code]) -> bool {
                $slice_eq(x,y)
            }
            fn ct_select(flag: bool, x: $code, y: $code) -> $code {
                $select(flag,x,y)
            }
            fn ct_copy(flag: bool, x: &mut [$code], y: &[$code]) {
                $copy(flag,x,y)
            }
        }
    }
}

impl_ConstantTime!(u8, ct_u8_eq, ct_u8_slice_eq, ct_select_u8, ct_copy_u8);
impl_ConstantTime!(u16, ct_u16_eq, ct_u16_slice_eq, ct_select_u16, ct_copy_u16);
impl_ConstantTime!(u32, ct_u32_eq, ct_u32_slice_eq, ct_select_u32, ct_copy_u32);
impl_ConstantTime!(u64, ct_u64_eq, ct_u64_slice_eq, ct_select_u64, ct_copy_u64);
impl_ConstantTime!(usize, ct_usize_eq, ct_usize_slice_eq, ct_select_usize, ct_copy_usize);


/*
 * The purpose of the below macro is two fold. 
 *  1. Define the function to do constant unsigned integer comparisons
 *  2. Define a *fairly* comphensive test to validate that function.
 *
 */
macro_rules! ct_eq_gen {
    ($name: ident, $code: ident, $($shr: expr),*
        ;; $test_name: ident, $test_v0: expr, $test_v1: expr) => {
        ///Tests if two values are equal in constant time.
        ///
        ///Completely avoids branching.
        #[no_mangle]
        #[inline(never)]
        pub extern "C" fn $name( x: $code, y: $code) -> bool {
            let mut z: $code = max!($code) ^ (x^y);
            $(
                z &= z.wrapping_shr($shr);
            )*
            /* 
             * Convert to a boolean
             * This is 99% syntax sugar
             * z will get moved eax about 5 instructions before this
             * The only operation done here is
             *
             *    andl $1, %eax
             *
             *  Which just asserts the structure of a boolean
             *  remain 0x01 or 0x00.
             */
            let val = z as u8;
            unsafe{trans::<u8,bool>(val)}
        }
        #[test]
        fn $test_name() {
            const MAX: $code = max!($code);
            let x: $code = $test_v0;
            let y: $code = $test_v1;
            assert_eq!( ct_eq(MAX,MAX), true);
            assert_eq!( ct_eq(x,x), true);
            assert_eq!( ct_eq(y,y), true);
            assert_eq!( ct_eq::<$code>(0,0), true);
            assert_eq!( ct_eq::<$code>(1,1), true);
            assert_eq!( ct_eq::<$code>(MAX,0), false);
            assert_eq!( ct_eq::<$code>(MAX,1), false);
            assert_eq!( ct_eq(MAX,x), false);
            assert_eq!( ct_eq(MAX,y), false);
            assert_eq!( ct_eq(y,1), false);
            assert_eq!( ct_eq(x,1), false);
            assert_eq!( ct_eq(y,0), false);
            assert_eq!( ct_eq(x,0), false);
            assert_eq!( ct_eq(x,y), false);
            $(
                assert_eq!( ct_eq::<$code>($shr,$shr), true);
                assert_eq!( ct_eq::<$code>($shr,0), false);
                assert_eq!( ct_eq::<$code>($shr,MAX), false);
            )*
        }
    }
}
ct_eq_gen!(ct_u8_eq,u8,4,2,1;;
    test_ct_u8_eq, 155, 15);
ct_eq_gen!(ct_u16_eq,u16,8,4,2,1;;
    test_ct_u16_eq, 32000, 5);
ct_eq_gen!(ct_u32_eq,u32,16,8,4,2,1;;
    test_ct_u32_eq, 2000000, 15);
ct_eq_gen!(ct_u64_eq,u64,32,16,8,4,2,1;;
    test_ct_u64_eq, 25893654215879, 2);
#[cfg(target_pointer_width = "32")]
ct_eq_gen!(ct_usize_eq,usize,16,8,4,2,1;;
    test_ct_u32_eq, 2082600, 15);
#[cfg(target_pointer_width = "64")]
ct_eq_gen!(ct_usize_eq,usize,32,16,8,4,2,1;;
    test_ct_usize_eq, 859632175648921456, 5);

macro_rules! ct_eq_slice_gen {
    ($name:ident,$code: ident;;$test_name:ident) => {
        ///Check the equality of slices.
        ///
        ///This will transverse the entire slice reguardless of if a
        ///conflict is found early or not. This way an external hacker
        ///can not guess the contents of a buffer byte by byte and 
        ///carefully measure the timing responses.
        #[no_mangle]
        pub extern "C" fn $name( x: &[$code], y: &[$code]) -> bool {
            let x_len = x.len();
            let y_len = y.len();
            if x_len != y_len {
               return false;
            }
            let y = &y[..x_len];    // elide bounds checks; see Rust commit 6a7bc47
            let mut flag: $code = 0;
            for i in 0..x_len {
                flag |= x[i] ^ y[i];
            }
            <$code as ConstantTime>::ct_eq(flag,0)
        }
        #[test]
        fn $test_name() {
            const MAX: $code = max!($code);
            let x: [$code;10] = [0,0,0,0,0,0,0,0,0,0];
            let y: [$code;10] = [MAX,MAX,MAX,MAX,MAX,MAX,MAX,MAX,MAX,MAX];
            let z: [$code;10] = [1,1,1,1,1,1,1,1,1,1];
            assert_eq!( ct_eq_slice( &x, &x), true);
            assert_eq!( ct_eq_slice( &y, &y), true);
            assert_eq!( ct_eq_slice( &z, &z), true);
            assert_eq!( ct_eq_slice( &x, &y), false);
            assert_eq!( ct_eq_slice( &x, &y), false);
            assert_eq!( ct_eq_slice( &y, &z), false);
        }
    }
}
ct_eq_slice_gen!(ct_u8_slice_eq,u8;;
    test_ct_u8_slice_eq);
ct_eq_slice_gen!(ct_u16_slice_eq,u16;;
    test_ct_u16_slice_eq);
ct_eq_slice_gen!(ct_u32_slice_eq,u32;;
    test_ct_u32_slice_eq);
ct_eq_slice_gen!(ct_u64_slice_eq,u64;;
    test_ct_u64_slice_eq);
ct_eq_slice_gen!(ct_usize_slice_eq,usize;;
    test_ct_usize_slice_eq);


macro_rules! ct_select_gen {
    ($name:ident,$code:ident;;$test_name:ident,$v0:expr,$v1:expr) => {
        ///Optional swapping.
        ///
        ///Allow to set a varible optionally at the same speed without
        ///branching, or changing speed.
        ///
        ///Returns X if flag == True.
        ///
        ///Returns Y if flag == False.
        ///
        ///At compile time this becomes a CMOV. This _is_ a brach.
        ///The branch misprediction cost is ~20cycles. And if this
        ///is incurred does not depend on the input, but the 
        ///random state of our machine + quantum winds.
        ///
        ///This should provide a consistent guarantee of speed.
        #[no_mangle]
        #[inline(never)]
        pub extern "C" fn $name(flag: bool, x: $code, y: $code) -> $code {
            let val: u8 = unsafe{trans::<bool,u8>(flag)};
            let flag = val as $code;
            ((max!($code) ^ flag.wrapping_sub(1))&x)|(flag.wrapping_sub(1)&y)
        }
        #[test]
        fn $test_name() {
            const MAX: $code = max!($code);
            assert_eq!( ct_select::<$code>(true,$v0,$v1), $v0);
            assert_eq!( ct_select::<$code>(false,$v0,$v1), $v1);
            assert_eq!( ct_select::<$code>(true,$v1,$v0), $v1);
            assert_eq!( ct_select::<$code>(false,$v1,$v0), $v0);
            assert_eq!( ct_select::<$code>(true,$v0,MAX), $v0);
            assert_eq!( ct_select::<$code>(false,$v0,MAX), MAX);
            assert_eq!( ct_select::<$code>(true,MAX,$v0), MAX);
            assert_eq!( ct_select::<$code>(false,MAX,$v0), $v0);
            assert_eq!( ct_select::<$code>(true,MAX,$v1), MAX);
            assert_eq!( ct_select::<$code>(false,MAX,$v1), $v1);
            assert_eq!( ct_select::<$code>(true,$v1,MAX), $v1);
            assert_eq!( ct_select::<$code>(false,$v1,MAX), MAX);
        }
    }
}

ct_select_gen!(ct_select_u8,u8;;
    test_ct_select_u8,155,4);
ct_select_gen!(ct_select_u16,u16;;
    test_ct_select_u16,30597,4);
ct_select_gen!(ct_select_u32,u32;;
    test_ct_select_u32,0x0DD74AA2,4);
ct_select_gen!(ct_select_u64,u64;;
    test_ct_select_u64,155,4);
ct_select_gen!(ct_select_usize,usize;;
    test_ct_select_usize,155,4);

macro_rules! ct_constant_copy_gen {
    ($name:ident,$code:ident
    ;;$test_name:ident,$sl_eq:ident,$other_test:ident) => {
        ///Optional buffer copying
        ///
        ///IF flag == True THEN X will be set to Y
        ///
        ///If flag == False THEN X is unchanged
        ///
        ///#Panic:
        ///
        ///This function will panic if X and Y are not equal length. 
        #[no_mangle]
        pub extern "C" fn $name(flag: bool, x: &mut [$code], y: &[$code]) {
            let x_len = x.len();
            let y_len = y.len();
            if x_len != y_len {
                panic!("Consistent Time: Attempted to copy between non-equal lens");
            }
            let y = &y[..x_len];    // elide bounds checks; see Rust commit 6a7bc47
            for i in 0..x_len {
                let y_temp = y[i].clone();
                let x_temp = x[i].clone();
                x[i] = <$code as ConstantTime>::ct_select(flag,y_temp,x_temp);
            }
        }
        #[test]
        fn $test_name() {
            const MAX: $code = max!($code);
            let base: [$code;10] = [0,0,0,0,0,0,0,0,0,0];
            let mut x: [$code;10] = [0,0,0,0,0,0,0,0,0,0];
            let y: [$code;10] = [MAX,MAX,MAX,MAX,MAX,MAX,MAX,MAX,MAX,MAX];
            ct_copy(false,&mut x, &y);
            assert_eq!( $sl_eq(&x,&base), true);
            ct_copy(true,&mut x, &y);
            assert_eq!( $sl_eq(&x,&base), false);
            assert_eq!( $sl_eq(&x,&y), true);
        }
        #[test]
        #[should_panic]
        fn $other_test() {
            let base: [$code;10] = [0,0,0,0,0,0,0,0,0,0];
            let mut x: [$code;9] = [0,0,0,0,0,0,0,0,0];
            //trigger panic
            //even on false evaluation
            //value of flag is irrelevant
            $name(false,&mut x,&base);
        }
    }
}
ct_constant_copy_gen!(ct_copy_u8,u8;;
    test_ct_copy_u8,ct_u8_slice_eq,test_ct_copy_u8_panic);
ct_constant_copy_gen!(ct_copy_u16,u16;;
    test_ct_copy_u16,ct_u16_slice_eq,test_ct_copy_u16_panic);
ct_constant_copy_gen!(ct_copy_u32,u32;;
    test_ct_copy_u32,ct_u32_slice_eq,test_ct_copy_u32_panic);
ct_constant_copy_gen!(ct_copy_u64,u64;;
    test_ct_copy_u64,ct_u64_slice_eq,test_ct_copy_u64_panic);
ct_constant_copy_gen!(ct_copy_usize,usize;;
    test_ct_copy_usize,ct_usize_slice_eq,test_ct_copy_usize_panic);
