// Copyright 2019, 2020 Wingchain
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::convert::TryFrom;

pub mod dsa;
pub mod errors;
pub mod hash;
pub mod address;

#[derive(PartialEq, Debug, Clone)]
pub enum HashLength {
	/// 160 bits
	HashLength20,

	/// 256 bits
	HashLength32,

	/// 512 bits
	HashLength64,
}

impl Into<usize> for HashLength {
	fn into(self) -> usize {
		match self {
			HashLength::HashLength20 => 20,
			HashLength::HashLength32 => 32,
			HashLength::HashLength64 => 64,
		}
	}
}

impl TryFrom<usize> for HashLength {
	type Error = errors::Error;

	#[inline]
	fn try_from(i: usize) -> Result<Self, Self::Error> {
		match i {
			20 => Ok(HashLength::HashLength20),
			32 => Ok(HashLength::HashLength32),
			64 => Ok(HashLength::HashLength64),
			other => Err(errors::ErrorKind::InvalidHashLength(other).into()),
		}
	}
}

#[derive(PartialEq, Debug, Clone)]
pub enum AddressLength {
	/// 160 bits
	AddressLength20,

	/// 256 bits
	AddressLength32,
}

impl Into<usize> for AddressLength {
	fn into(self) -> usize {
		match self {
			AddressLength::AddressLength20 => 20,
			AddressLength::AddressLength32 => 32,
		}
	}
}

impl TryFrom<usize> for AddressLength {
	type Error = errors::Error;

	#[inline]
	fn try_from(i: usize) -> Result<Self, Self::Error> {
		match i {
			20 => Ok(AddressLength::AddressLength20),
			32 => Ok(AddressLength::AddressLength32),
			other => Err(errors::ErrorKind::InvalidAddressLength(other).into()),
		}
	}
}
