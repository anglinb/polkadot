// Copyright 2017 Parity Technologies (UK) Ltd.
// This file is part of Substrate Demo.

// Substrate Demo is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate Demo is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate Demo.  If not, see <http://www.gnu.org/licenses/>.

//! Primitives for the runtime modules.

use rstd::prelude::*;
use rstd::{self, result};
use runtime_io;
#[cfg(feature = "std")] use std::fmt::{Debug, Display};
#[cfg(feature = "std")] use serde::{Serialize, de::DeserializeOwned};
use substrate_primitives;
use codec::Slicable;
pub use integer_sqrt::IntegerSquareRoot;
pub use num_traits::{Zero, One, Bounded};
use rstd::ops::{Add, Sub, Mul, Div, Rem, AddAssign, SubAssign, MulAssign, DivAssign, RemAssign};

/// A lazy value.
pub trait Lazy<T: ?Sized> {
	fn get(&mut self) -> &T;
}

/// Means of signature verification.
pub trait Verify {
	/// Type of the signer.
	type Signer;
	/// Verify a signature.
	fn verify<L: Lazy<[u8]>>(&self, msg: L, signer: &Self::Signer) -> bool;
}

/// Means of changing one type into another in a manner dependent on the source type.
pub trait AuxLookup {
	/// Type to lookup from.
	type Source;
	/// Type to lookup into.
	type Target;
	/// Attempt a lookup.
	fn lookup(s: Self::Source) -> result::Result<Self::Target, &'static str>;
}

/// Simple payment making trait, operating on a single generic `AccountId` type.
pub trait MakePayment<AccountId> {
	/// Make some sort of payment concerning `who` for an extrinsic (transaction) of encoded length
	/// `encoded_len` bytes. Return true iff the payment was successful.
	fn make_payment(who: &AccountId, encoded_len: usize) -> Result<(), &'static str>;
}

impl<T> MakePayment<T> for () {
	fn make_payment(_: &T, _: usize) -> Result<(), &'static str> { Ok(()) }
}

/// Extensible conversion trait. Generic over both source and destination types.
pub trait Convert<A, B> {
	/// Make conversion.
	fn convert(a: A) -> B;
}

/// Simple trait similar to `Into`, except that it can be used to convert numerics between each
/// other.
pub trait As<T> {
	/// Convert forward (ala `Into::into`).
	fn as_(self) -> T;
	/// Convert backward (ala `From::from`).
	fn sa(T) -> Self;
}

macro_rules! impl_numerics {
	( $( $t:ty ),* ) => {
		$(
			impl_numerics!($t: u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize,);
		)*
	};
	( $f:ty : $t:ty, $( $rest:ty, )* ) => {
		impl As<$t> for $f {
			fn as_(self) -> $t { self as $t }
			fn sa(t: $t) -> Self { t as Self }
		}
		impl_numerics!($f: $( $rest, )*);
	};
	( $f:ty : ) => {}
}

impl_numerics!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

pub struct Identity;
impl<T> Convert<T, T> for Identity {
	fn convert(a: T) -> T { a }
}

pub trait MaybeEmpty {
	fn is_empty(&self) -> bool;
}

impl<T: Default + PartialEq> MaybeEmpty for T {
	fn is_empty(&self) -> bool {
		*self == T::default()
	}
}

pub trait HasPublicAux {
	type PublicAux: MaybeEmpty;
}

pub trait RefInto<T> {
	fn ref_into(&self) -> &T;
}
impl<T> RefInto<T> for T {
	fn ref_into(&self) -> &T { &self }
}

pub trait SimpleArithmetic:
	Zero + One + IntegerSquareRoot + As<usize> +
	Add<Self, Output = Self> + AddAssign<Self> +
	Sub<Self, Output = Self> + SubAssign<Self> +
	Mul<Self, Output = Self> + MulAssign<Self> +
	Div<Self, Output = Self> + DivAssign<Self> +
	Rem<Self, Output = Self> + RemAssign<Self> +
	PartialOrd<Self> + Ord
{}
impl<T:
	Zero + One + IntegerSquareRoot + As<usize> +
	Add<Self, Output = Self> + AddAssign<Self> +
	Sub<Self, Output = Self> + SubAssign<Self> +
	Mul<Self, Output = Self> + MulAssign<Self> +
	Div<Self, Output = Self> + DivAssign<Self> +
	Rem<Self, Output = Self> + RemAssign<Self> +
	PartialOrd<Self> + Ord
> SimpleArithmetic for T {}

/// Trait for things that can be clear (have no bits set). For numeric types, essentially the same
/// as `Zero`.
pub trait Clear {
	/// True iff no bits are set.
	fn is_clear(&self) -> bool;

	/// Return the value of Self that is clear.
	fn clear() -> Self;
}

impl<T: Default + Eq + PartialEq> Clear for T {
	fn is_clear(&self) -> bool { *self == Self::clear() }
	fn clear() -> Self { Default::default() }
}

pub trait SimpleBitOps:
	Sized + Clear +
	rstd::ops::BitOr<Self, Output = Self> +
	rstd::ops::BitAnd<Self, Output = Self>
{}
impl<T:
	Sized + Clear +
	rstd::ops::BitOr<Self, Output = Self> +
	rstd::ops::BitAnd<Self, Output = Self>
> SimpleBitOps for T {}

/// Something that can be executed.
pub trait Executable {
	fn execute();
}

impl Executable for () {
	fn execute() {}
}
impl<A: Executable, B: Executable> Executable for (A, B) {
	fn execute() {
		A::execute();
		B::execute();
	}
}

/// Abstraction around hashing
pub trait Hashing: 'static + MaybeSerializeDebug + Clone + Eq + PartialEq {	// Stupid bug in the Rust compiler believes derived
																	// traits must be fulfilled by all type parameters.
	/// The hash type produced.
	type Output: Member + AsRef<[u8]>;

	/// Produce the hash of some byte-slice.
	fn hash(s: &[u8]) -> Self::Output;

	/// Produce the hash of some codec-encodable value.
	fn hash_of<S: Slicable>(s: &S) -> Self::Output {
		Slicable::using_encoded(s, Self::hash)
	}

	/// Produce the patricia-trie root of a mapping from indices to byte slices.
	fn enumerated_trie_root(items: &[&[u8]]) -> Self::Output;

	/// Iterator-based version of `enumerated_trie_root`.
	fn ordered_trie_root<
		I: IntoIterator<Item = A>,
		A: AsRef<[u8]>
	>(input: I) -> Self::Output;

	/// The Patricia tree root of the given mapping as an iterator.
	fn trie_root<
		I: IntoIterator<Item = (A, B)>,
		A: AsRef<[u8]> + Ord,
		B: AsRef<[u8]>
	>(input: I) -> Self::Output;

	/// Acquire the global storage root.
	fn storage_root() -> Self::Output;
}

/// Blake2-256 Hashing implementation.
#[derive(PartialEq, Eq, Clone)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
pub struct BlakeTwo256;

impl Hashing for BlakeTwo256 {
	type Output = substrate_primitives::H256;
	fn hash(s: &[u8]) -> Self::Output {
		runtime_io::blake2_256(s).into()
	}
	fn enumerated_trie_root(items: &[&[u8]]) -> Self::Output {
		runtime_io::enumerated_trie_root(items).into()
	}
	fn trie_root<
		I: IntoIterator<Item = (A, B)>,
		A: AsRef<[u8]> + Ord,
		B: AsRef<[u8]>
	>(input: I) -> Self::Output {
		runtime_io::trie_root(input).into()
	}
	fn ordered_trie_root<
		I: IntoIterator<Item = A>,
		A: AsRef<[u8]>
	>(input: I) -> Self::Output {
		runtime_io::ordered_trie_root(input).into()
	}
	fn storage_root() -> Self::Output {
		runtime_io::storage_root().into()
	}
}

/// Something that can be checked for equality and printed out to a debug channel if bad.
pub trait CheckEqual {
	fn check_equal(&self, other: &Self);
}

impl CheckEqual for substrate_primitives::H256 {
	#[cfg(feature = "std")]
	fn check_equal(&self, other: &Self) {
		use substrate_primitives::hexdisplay::HexDisplay;
		if &self.0 != &other.0 {
			println!("Hash: given={}, expected={}", HexDisplay::from(&self.0), HexDisplay::from(&other.0));
		}
	}

	#[cfg(not(feature = "std"))]
	fn check_equal(&self, other: &Self) {
		if self != other {
			runtime_io::print("Hash not equal");
			runtime_io::print(&self.0[..]);
			runtime_io::print(&other.0[..]);
		}
	}
}

#[cfg(feature = "std")]
pub trait MaybeSerializeDebugButNotDeserialize: Serialize + Debug {}
#[cfg(feature = "std")]
impl<T: Serialize + Debug> MaybeSerializeDebugButNotDeserialize for T {}

#[cfg(not(feature = "std"))]
pub trait MaybeSerializeDebugButNotDeserialize {}
#[cfg(not(feature = "std"))]
impl<T> MaybeSerializeDebugButNotDeserialize for T {}

#[cfg(feature = "std")]
pub trait MaybeSerializeDebug: Serialize + DeserializeOwned + Debug {}
#[cfg(feature = "std")]
impl<T: Serialize + DeserializeOwned + Debug> MaybeSerializeDebug for T {}

#[cfg(not(feature = "std"))]
pub trait MaybeSerializeDebug {}
#[cfg(not(feature = "std"))]
impl<T> MaybeSerializeDebug for T {}

#[cfg(feature = "std")]
pub trait MaybeDisplay: Display {}
#[cfg(feature = "std")]
impl<T: Display> MaybeDisplay for T {}

#[cfg(not(feature = "std"))]
pub trait MaybeDisplay {}
#[cfg(not(feature = "std"))]
impl<T> MaybeDisplay for T {}

pub trait Member: Send + Sync + Sized + MaybeSerializeDebug + Eq + PartialEq + Clone + 'static {}
impl<T: Send + Sync + Sized + MaybeSerializeDebug + Eq + PartialEq + Clone + 'static> Member for T {}

/// Something that acts like a `Digest` - it can have `Log`s `push`ed onto it and these `Log`s are
/// each `Slicable`.
pub trait Digest {
	type Item: Member;
	fn push(&mut self, item: Self::Item);
}

/// Something which fulfills the abstract idea of a Substrate header. It has types for a `Number`,
/// a `Hash` and a `Digest`. It provides access to an `extrinsics_root`, `state_root` and
/// `parent_hash`, as well as a `digest` and a block `number`.
///
/// You can also create a `new` one from those fields.
pub trait Header: Clone + Send + Sync + Slicable + Eq + MaybeSerializeDebug {
	type Number: Member + ::rstd::hash::Hash + Copy + MaybeDisplay + SimpleArithmetic + Slicable;
	type Hash: Member + ::rstd::hash::Hash + Copy + MaybeDisplay + Default + SimpleBitOps + Slicable + AsRef<[u8]>;
	type Hashing: Hashing<Output = Self::Hash>;
	type Digest: Member + Default;

	fn new(
		number: Self::Number,
		extrinsics_root: Self::Hash,
		state_root: Self::Hash,
		parent_hash: Self::Hash,
		digest: Self::Digest
	) -> Self;

	fn number(&self) -> &Self::Number;
	fn set_number(&mut self, Self::Number);

	fn extrinsics_root(&self) -> &Self::Hash;
	fn set_extrinsics_root(&mut self, Self::Hash);

	fn state_root(&self) -> &Self::Hash;
	fn set_state_root(&mut self, Self::Hash);

	fn parent_hash(&self) -> &Self::Hash;
	fn set_parent_hash(&mut self, Self::Hash);

	fn digest(&self) -> &Self::Digest;
	fn set_digest(&mut self, Self::Digest);

	fn hash(&self) -> Self::Hash {
		<Self::Hashing as Hashing>::hash_of(self)
	}
}

/// Something which fulfills the abstract idea of a Substrate block. It has types for an
/// `Extrinsic` piece of information as well as a `Header`.
///
/// You can get an iterator over each of the `extrinsics` and retrieve the `header`.
pub trait Block: Clone + Send + Sync + Slicable + Eq + MaybeSerializeDebug {
	type Extrinsic: Member + Slicable;
	type Header: Header<Hash=Self::Hash>;
	type Hash: Member + ::rstd::hash::Hash + Copy + MaybeDisplay + Default + SimpleBitOps + Slicable + AsRef<[u8]>;

	fn header(&self) -> &Self::Header;
	fn extrinsics(&self) -> &[Self::Extrinsic];
	fn deconstruct(self) -> (Self::Header, Vec<Self::Extrinsic>);
	fn new(header: Self::Header, extrinsics: Vec<Self::Extrinsic>) -> Self;
	fn hash(&self) -> Self::Hash {
		<<Self::Header as Header>::Hashing as Hashing>::hash_of(self.header())
	}
}

/// Extract the hashing type for a block.
pub type HashingFor<B> = <<B as Block>::Header as Header>::Hashing;

/// A "checkable" piece of information, used by the standard Substrate Executive in order to
/// check the validity of a piece of extrinsic information, usually by verifying the signature.
pub trait Checkable: Sized + Send + Sync {
	type Address: Member + MaybeDisplay;
	type AccountId: Member + MaybeDisplay;
	type Checked: Member;
	fn sender(&self) -> &Self::Address;
	fn check<ThisLookup: FnOnce(Self::Address) -> Result<Self::AccountId, &'static str> + Send + Sync>(self, lookup: ThisLookup) -> Result<Self::Checked, &'static str>;
}

/// A "checkable" piece of information, used by the standard Substrate Executive in order to
/// check the validity of a piece of extrinsic information, usually by verifying the signature.
///
/// This does that checking without requiring a lookup argument. 
pub trait BlindCheckable: Sized + Send + Sync {
	type Address: Member + MaybeDisplay;
	type Checked: Member;
	fn sender(&self) -> &Self::Address;
	fn check(self) -> Result<Self::Checked, &'static str>;
}

impl<T: BlindCheckable> Checkable for T {
	type Address = <Self as BlindCheckable>::Address;
	type AccountId = <Self as BlindCheckable>::Address;
	type Checked = <Self as BlindCheckable>::Checked;
	fn sender(&self) -> &Self::Address { BlindCheckable::sender(self) }
	fn check<ThisLookup: FnOnce(Self::Address) -> Result<Self::AccountId, &'static str> + Send + Sync>(self, _: ThisLookup) -> Result<Self::Checked, &'static str> { BlindCheckable::check(self) }
}

/// An "executable" piece of information, used by the standard Substrate Executive in order to
/// enact a piece of extrinsic information by marshalling and dispatching to a named functioon
/// call.
///
/// Also provides information on to whom this information is attributable and an index that allows
/// each piece of attributable information to be disambiguated.
pub trait Applyable: Sized + Send + Sync {
	type AccountId: Member + MaybeDisplay;
	type Index: Member + MaybeDisplay + SimpleArithmetic;
	fn index(&self) -> &Self::Index;
	fn sender(&self) -> &Self::AccountId;
	fn apply(self) -> Result<(), &'static str>;
}
